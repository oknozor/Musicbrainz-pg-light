use serde_json::Value;
use sqlx::{Postgres, Transaction, prelude::FromRow};
use std::fmt;

use crate::{MbLight, MbLightError, error::MbLightResult, settings::MbLightSettingsExt};

#[derive(FromRow, Debug)]
pub struct PendingData {
    fulltable: String,
    op: Operation,
    pub xid: i64,
    olddata: Option<Value>,
    newdata: Option<Value>,
    keys: Vec<String>,
}

#[derive(sqlx::Type, Debug)]
#[repr(i8)]
pub enum Operation {
    Delete = b'd' as i8,
    Insert = b'i' as i8,
    Update = b'u' as i8,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Delete => write!(f, "DELETE"),
            Operation::Insert => write!(f, "INSERT"),
            Operation::Update => write!(f, "UPDATE"),
        }
    }
}

impl PendingData {
    pub async fn all(db: &sqlx::PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as(
            r#"SELECT pd.xid,
                   pd.tablename as fulltable,
                   pd.op,
                   pk.keys,
                   pd.olddata,
                   pd.newdata
              FROM dbmirror2.pending_data pd
              JOIN dbmirror2.pending_keys pk
                ON pk.tablename = pd.tablename
                 ORDER BY pd.xid, pd.seqid"#,
        )
        .fetch_all(db)
        .await
    }

    pub async fn remove_by_xid(
        mut tx: Transaction<'_, Postgres>,
        xid: i64,
    ) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        sqlx::query(r#"DELETE FROM dbmirror2.pending_data WHERE xid = $1"#)
            .bind(xid)
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    pub fn to_sql_inline(&self) -> MbLightResult<Option<String>> {
        let (schema, table) = self.split_table_schema();
        match self.op {
            Operation::Insert => {
                let obj = self
                    .newdata
                    .as_ref()
                    .ok_or_else(|| MbLightError::MissingPendingData("newdata"))?;
                let obj = obj.as_object().unwrap();
                let col_names: Vec<&str> = obj.keys().map(|k| k.as_str()).collect();
                let col_values: Vec<String> = obj.values().map(sql_literal).collect();
                Ok(Some(format!(
                    r#"INSERT INTO "{schema}"."{table}" ({}) VALUES ({});"#,
                    col_names.join(", "),
                    col_values.join(", ")
                )))
            }
            Operation::Update => {
                let Some(set_clause) = self.get_set_clause()? else {
                    return Ok(None);
                };

                let where_clause = self.get_where_clause()?;

                Ok(Some(format!(
                    r#"UPDATE "{schema}"."{table}" SET {set_clause} WHERE {where_clause};"#,
                )))
            }
            Operation::Delete => {
                let where_clause = self.get_where_clause()?;

                Ok(Some(format!(
                    r#"DELETE FROM "{schema}"."{table}" WHERE {where_clause};"#,
                )))
            }
        }
    }

    pub fn split_table_schema(&self) -> (&str, &str) {
        let parts: Vec<&str> = self.fulltable.split('.').collect();
        (parts[0], parts[1])
    }

    fn get_where_clause(&self) -> MbLightResult<String> {
        let old_obj = self
            .olddata
            .as_ref()
            .ok_or_else(|| MbLightError::MissingPendingData("olddata"))?
            .as_object()
            .ok_or_else(|| MbLightError::MalformedPendingData("olddata"))?;

        let where_clause: Vec<String> = old_obj
            .iter()
            .filter(|(k, _)| self.sanitized_keys().contains(&k.as_str()))
            .map(|(k, v)| format!("{k} = {}", sql_literal(v)))
            .collect();

        Ok(where_clause.join(" AND "))
    }

    fn get_set_clause(&self) -> MbLightResult<Option<String>> {
        let new_obj = self
            .newdata
            .as_ref()
            .ok_or_else(|| MbLightError::MissingPendingData("newdata"))?;

        let new_obj = new_obj.as_object().unwrap();

        let old_obj = self
            .olddata
            .as_ref()
            .ok_or_else(|| MbLightError::MissingPendingData("olddata"))?
            .as_object()
            .ok_or_else(|| MbLightError::MalformedPendingData("olddata"))?;

        let mut changes = Vec::new();

        for (k, new_val) in new_obj {
            if let Some(old_val) = old_obj.get(k) {
                if old_val != new_val {
                    changes.push(format!(r#"{k} = {}"#, sql_literal(new_val)));
                }
            } else {
                changes.push(format!(r#"{k} = {}"#, sql_literal(new_val)));
            }
        }

        if changes.is_empty() {
            return Ok(None);
        }

        Ok(Some(changes.join(", ")))
    }

    fn sanitized_keys(&self) -> Vec<&str> {
        self.keys
            .iter()
            .map(|key| key.trim_matches(['{', '}']))
            .collect()
    }
}

fn sql_literal(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "NULL".to_string(),
        serde_json::Value::Array(arr) => {
            let elems: Vec<String> = arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::String(s) => s.replace('\'', "''"),
                    _ => panic!("unsupported array element: {:?}", v),
                })
                .collect();
            format!("'{{{}}}'::integer[]", elems.join(","))
        }
        serde_json::Value::Object(_) => {
            format!("'{}'", val.to_string().replace('\'', "''"))
        }
    }
}

impl<S: MbLightSettingsExt> MbLight<S> {
    pub async fn truncate_pending_data(&self) -> Result<(), sqlx::Error> {
        sqlx::query(r#"TRUNCATE TABLE dbmirror2.pending_data"#)
            .execute(&self.db)
            .await?;

        sqlx::query(r#"TRUNCATE TABLE dbmirror2.pending_keys"#)
            .execute(&self.db)
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::error::MbLightResult;

    use super::*;

    #[test]
    fn test_update_release_group_meta_no_changes() -> MbLightResult<()> {
        let olddata = serde_json::json!({
            "id": 3183888,
            "release_count": 3,
            "first_release_date_year": 2022,
            "first_release_date_month": 9,
            "first_release_date_day": 30,
            "rating": null,
            "rating_count": null
        });

        let newdata = serde_json::json!({
            "id": 3183888,
            "release_count": 3,
            "first_release_date_year": 2022,
            "first_release_date_month": 9,
            "first_release_date_day": 30,
            "rating": null,
            "rating_count": null
        });

        let pd = PendingData {
            fulltable: "musicbrainz.release_group_meta".to_string(),
            op: Operation::Update,
            xid: 2266759644,
            olddata: Some(olddata),
            newdata: Some(newdata),
            keys: vec!["id".to_string()],
        };

        let query = pd.to_sql_inline()?;

        assert!(query.is_none());
        Ok(())
    }
}
