//! Catalog database operations.

use rusqlite::{params, OptionalExtension};

use super::{Database, DbError, DbResult};
use crate::models::CatalogItem;

impl Database {
    /// Insert or update a catalog item.
    pub fn upsert_catalog_item(&self, item: &CatalogItem) -> DbResult<()> {
        let aliases_json = serde_json::to_string(&item.aliases)?;
        let species_json = serde_json::to_string(&item.species)?;
        let routes_json = serde_json::to_string(&item.routes)?;
        let dose_range_json = item
            .dose_range
            .as_ref()
            .map(|dr| serde_json::to_string(dr))
            .transpose()?;

        self.conn.execute(
            r#"
            INSERT INTO inventory_catalog (
                sku, name, aliases, concentration, package_size,
                species, routes, dose_range, active, server_id, last_synced, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))
            ON CONFLICT(sku) DO UPDATE SET
                name = excluded.name,
                aliases = excluded.aliases,
                concentration = excluded.concentration,
                package_size = excluded.package_size,
                species = excluded.species,
                routes = excluded.routes,
                dose_range = excluded.dose_range,
                active = excluded.active,
                server_id = excluded.server_id,
                last_synced = excluded.last_synced,
                updated_at = datetime('now')
            "#,
            params![
                item.sku,
                item.name,
                aliases_json,
                item.concentration,
                item.package_size,
                species_json,
                routes_json,
                dose_range_json,
                item.active,
                item.server_id,
                item.last_synced,
            ],
        )?;
        Ok(())
    }

    /// Get a catalog item by SKU.
    pub fn get_catalog_item(&self, sku: &str) -> DbResult<Option<CatalogItem>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT sku, name, aliases, concentration, package_size,
                       species, routes, dose_range, active, server_id, last_synced
                FROM inventory_catalog
                WHERE sku = ?
                "#,
                [sku],
                |row| {
                    Ok(CatalogItemRow {
                        sku: row.get(0)?,
                        name: row.get(1)?,
                        aliases: row.get(2)?,
                        concentration: row.get(3)?,
                        package_size: row.get(4)?,
                        species: row.get(5)?,
                        routes: row.get(6)?,
                        dose_range: row.get(7)?,
                        active: row.get(8)?,
                        server_id: row.get(9)?,
                        last_synced: row.get(10)?,
                    })
                },
            )
            .optional()?;

        result.map(|row| row.try_into()).transpose()
    }

    /// Search catalog using FTS5 (BM25 ranking).
    pub fn search_catalog(&self, query: &str, limit: usize) -> DbResult<Vec<CatalogItem>> {
        // Escape special FTS5 characters and add prefix matching
        let escaped_query = escape_fts_query(query);

        let mut stmt = self.conn.prepare(
            r#"
            SELECT c.sku, c.name, c.aliases, c.concentration, c.package_size,
                   c.species, c.routes, c.dose_range, c.active, c.server_id, c.last_synced,
                   bm25(inventory_catalog_fts) as rank
            FROM inventory_catalog c
            JOIN inventory_catalog_fts fts ON c.rowid = fts.rowid
            WHERE inventory_catalog_fts MATCH ?
            AND c.active = 1
            ORDER BY rank
            LIMIT ?
            "#,
        )?;

        let rows = stmt.query_map(params![escaped_query, limit as i64], |row| {
            Ok(CatalogItemRow {
                sku: row.get(0)?,
                name: row.get(1)?,
                aliases: row.get(2)?,
                concentration: row.get(3)?,
                package_size: row.get(4)?,
                species: row.get(5)?,
                routes: row.get(6)?,
                dose_range: row.get(7)?,
                active: row.get(8)?,
                server_id: row.get(9)?,
                last_synced: row.get(10)?,
            })
        })?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?.try_into()?);
        }
        Ok(items)
    }

    /// Get all active catalog items.
    pub fn list_catalog_items(&self, active_only: bool) -> DbResult<Vec<CatalogItem>> {
        let sql = if active_only {
            r#"
            SELECT sku, name, aliases, concentration, package_size,
                   species, routes, dose_range, active, server_id, last_synced
            FROM inventory_catalog
            WHERE active = 1
            ORDER BY name
            "#
        } else {
            r#"
            SELECT sku, name, aliases, concentration, package_size,
                   species, routes, dose_range, active, server_id, last_synced
            FROM inventory_catalog
            ORDER BY name
            "#
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(CatalogItemRow {
                sku: row.get(0)?,
                name: row.get(1)?,
                aliases: row.get(2)?,
                concentration: row.get(3)?,
                package_size: row.get(4)?,
                species: row.get(5)?,
                routes: row.get(6)?,
                dose_range: row.get(7)?,
                active: row.get(8)?,
                server_id: row.get(9)?,
                last_synced: row.get(10)?,
            })
        })?;

        let mut items = Vec::new();
        for row in rows {
            items.push(row?.try_into()?);
        }
        Ok(items)
    }

    /// Delete a catalog item.
    pub fn delete_catalog_item(&self, sku: &str) -> DbResult<bool> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM inventory_catalog WHERE sku = ?", [sku])?;
        Ok(rows_affected > 0)
    }

    /// Mark item as inactive (soft delete).
    pub fn deactivate_catalog_item(&self, sku: &str) -> DbResult<bool> {
        let rows_affected = self.conn.execute(
            "UPDATE inventory_catalog SET active = 0, updated_at = datetime('now') WHERE sku = ?",
            [sku],
        )?;
        Ok(rows_affected > 0)
    }
}

/// Intermediate row struct for database mapping.
struct CatalogItemRow {
    sku: String,
    name: String,
    aliases: String,
    concentration: Option<String>,
    package_size: Option<String>,
    species: String,
    routes: String,
    dose_range: Option<String>,
    active: bool,
    server_id: Option<String>,
    last_synced: Option<String>,
}

impl TryFrom<CatalogItemRow> for CatalogItem {
    type Error = DbError;

    fn try_from(row: CatalogItemRow) -> Result<Self, Self::Error> {
        Ok(CatalogItem {
            sku: row.sku,
            name: row.name,
            aliases: serde_json::from_str(&row.aliases)?,
            concentration: row.concentration,
            package_size: row.package_size,
            species: serde_json::from_str(&row.species)?,
            routes: serde_json::from_str(&row.routes)?,
            dose_range: row
                .dose_range
                .map(|s| serde_json::from_str(&s))
                .transpose()?,
            active: row.active,
            server_id: row.server_id,
            last_synced: row.last_synced,
        })
    }
}

/// Escape special FTS5 characters and prepare query for prefix matching.
fn escape_fts_query(query: &str) -> String {
    // Remove special FTS5 operators and add wildcard for prefix matching
    let cleaned: String = query
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();

    // Add prefix matching operator
    cleaned
        .split_whitespace()
        .map(|word| format!("{}*", word))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DoseRange;

    fn setup_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn test_upsert_and_get() {
        let db = setup_db();

        let mut item = CatalogItem::new("SKU001".into(), "Carprofen 100mg".into());
        item.aliases = vec!["rimadyl".into(), "novox".into()];
        item.species = vec!["canine".into(), "feline".into()];
        item.routes = vec!["PO".into()];
        item.concentration = Some("100mg".into());

        db.upsert_catalog_item(&item).unwrap();

        let retrieved = db.get_catalog_item("SKU001").unwrap().unwrap();
        assert_eq!(retrieved.name, "Carprofen 100mg");
        assert_eq!(retrieved.aliases, vec!["rimadyl", "novox"]);
        assert_eq!(retrieved.species, vec!["canine", "feline"]);
    }

    #[test]
    fn test_upsert_updates() {
        let db = setup_db();

        let mut item = CatalogItem::new("SKU001".into(), "Original Name".into());
        db.upsert_catalog_item(&item).unwrap();

        item.name = "Updated Name".into();
        db.upsert_catalog_item(&item).unwrap();

        let retrieved = db.get_catalog_item("SKU001").unwrap().unwrap();
        assert_eq!(retrieved.name, "Updated Name");
    }

    #[test]
    fn test_search_catalog() {
        let db = setup_db();

        let mut item1 = CatalogItem::new("SKU001".into(), "Carprofen 100mg tablets".into());
        item1.aliases = vec!["rimadyl".into()];
        db.upsert_catalog_item(&item1).unwrap();

        let mut item2 = CatalogItem::new("SKU002".into(), "Meloxicam 1.5mg/mL".into());
        item2.aliases = vec!["metacam".into()];
        db.upsert_catalog_item(&item2).unwrap();

        // Search by name
        let results = db.search_catalog("carprofen", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].sku, "SKU001");

        // Search by alias
        let results = db.search_catalog("rimadyl", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].sku, "SKU001");

        // Prefix search
        let results = db.search_catalog("carp", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_deactivate() {
        let db = setup_db();

        let item = CatalogItem::new("SKU001".into(), "Test Drug".into());
        db.upsert_catalog_item(&item).unwrap();

        db.deactivate_catalog_item("SKU001").unwrap();

        // Should not appear in search
        let results = db.search_catalog("test", 10).unwrap();
        assert_eq!(results.len(), 0);

        // Should still be retrievable directly
        let item = db.get_catalog_item("SKU001").unwrap().unwrap();
        assert!(!item.active);
    }

    #[test]
    fn test_dose_range_persistence() {
        let db = setup_db();

        let mut item = CatalogItem::new("SKU001".into(), "Test Drug".into());
        item.dose_range = Some(DoseRange {
            min_dose_per_kg: 1.0,
            max_dose_per_kg: 5.0,
            unit: "mg".into(),
        });
        db.upsert_catalog_item(&item).unwrap();

        let retrieved = db.get_catalog_item("SKU001").unwrap().unwrap();
        let range = retrieved.dose_range.unwrap();
        assert_eq!(range.min_dose_per_kg, 1.0);
        assert_eq!(range.max_dose_per_kg, 5.0);
        assert_eq!(range.unit, "mg");
    }
}
