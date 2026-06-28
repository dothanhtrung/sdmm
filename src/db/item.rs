use indexmap::IndexSet;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteQueryResult;

#[derive(sqlx::FromRow, Eq, PartialEq, Hash)]
pub struct Item {
    pub id: i64,
    pub name: Option<String>,
    pub path: String,
    pub base_label: String,
    pub note: String,
}

pub async fn mark_obsolete_all(pool: &SqlitePool) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query!(r#"UPDATE item SET is_checked = false WHERE is_checked = true AND path != ''"#)
        .execute(pool)
        .await
}

/// Return (path, label)
pub async fn mark_obsolete(pool: &SqlitePool, id: i64) -> Result<(String, String), sqlx::Error> {
    sqlx::query!(r#"UPDATE item SET is_checked = false WHERE id = ?"#, id)
        .execute(pool)
        .await?;

    struct Temp {
        path: String,
        base_label: String,
    }
    let ret = sqlx::query_as!(Temp, r#"SELECT path, base_label FROM item WHERE id = ?"#, id)
        .fetch_one(pool)
        .await?;

    Ok((ret.path, ret.base_label))
}

pub async fn insert_or_update(
    pool: &SqlitePool,
    name: Option<&str>,
    path: &str,
    base_label: &str,
    blake3: &str,
    updated_at_ms: i64,
) -> Result<i64, sqlx::Error> {
    let ret_id = sqlx::query!(
        r#"
        INSERT INTO item (name, path, base_label, blake3, updated_at) VALUES (?, ?, ?, ?, ?)
        ON CONFLICT (path, base_label) DO UPDATE SET
            is_checked=true,
            blake3=excluded.blake3,
            base_label=excluded.base_label,
            name=excluded.name,
            updated_at = excluded.updated_at
        RETURNING id"#,
        name,
        path,
        base_label,
        blake3,
        updated_at_ms,
    )
    .fetch_one(pool)
    .await?
    .id;

    Ok(ret_id)
}

pub async fn clean(pool: &SqlitePool) -> Result<u64, sqlx::Error> {
    let count = sqlx::query!(r#"DELETE FROM item WHERE is_checked = false"#)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(count)
}

pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Item, sqlx::Error> {
    let item = sqlx::query_as!(
        Item,
        "SELECT id, name, path, base_label, note FROM item WHERE id = ?",
        id
    )
    .fetch_one(pool)
    .await?;

    Ok(item)
}

pub async fn search(
    pool: &SqlitePool,
    search: &str,
    limit: i64,
    offset: i64,
    tag_only: bool,
    duplicate_only: bool,
) -> Result<(Vec<Item>, i64), sqlx::Error> {
    //TODO: Search in note too
    let mut items = IndexSet::new();
    let mut count = 0;
    let limit_dup_count = if duplicate_only { 1 } else { 0 };

    if !tag_only {
        let items_by_name = sqlx::query_as!(
            Item,
            r#"SELECT id,name, path, base_label, note
            FROM item
            WHERE is_checked = true
                AND (name COLLATE NOCASE LIKE '%' || ? || '%'
                    OR model_name COLLATE NOCASE LIKE '%' || ? || '%')
                AND blake3 IN (
                    SELECT blake3 FROM item
                    WHERE is_checked = true
                    GROUP BY blake3
                    HAVING COUNT(*) > ?)
            ORDER BY updated_at DESC
            LIMIT ? OFFSET ?"#,
            search,
            search,
            limit_dup_count,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;

        let count_by_name: i64 = sqlx::query_scalar!(
            r#"SELECT count(id)
            FROM item
            WHERE is_checked = true
                AND (name COLLATE NOCASE LIKE '%' || ? || '%'
                    OR model_name COLLATE NOCASE LIKE '%' || ? || '%')
                AND blake3 IN (
                    SELECT blake3 FROM item
                    WHERE is_checked = true
                    GROUP BY blake3
                    HAVING COUNT(*) > ?)"#,
            search,
            search,
            limit_dup_count,
        )
        .fetch_one(pool)
        .await?;

        items.extend(items_by_name);
        count += count_by_name;
    }

    let tags: Vec<String> = search
        .split_whitespace()
        .map(|s| s.to_string().to_lowercase())
        .collect();

    // WORKAROUND: Do not exclude name match if tag_only
    let search = if tag_only { "ikjsdfh3280urkjhfskjaeoiosd92304q31#!@&$^%@#&$*6" } else { search };

    if !tags.is_empty() {
        let search_by_tags = sqlx::query_as!(
            Item,
            r#"
            SELECT item.id as id, item.name as name, item.note as note, item.path as path, item.base_label as base_label
            FROM item
            LEFT JOIN tag_item ON item.id = tag_item.item
            LEFT JOIN tag ON tag.id = tag_item.tag
            WHERE item.is_checked = true
                AND tag.name IN (SELECT value FROM json_each(?))
                AND NOT(item.name COLLATE NOCASE LIKE '%' || ? || '%'
                        OR item.model_name COLLATE NOCASE LIKE '%' || ? || '%')
                AND blake3 IN (
                    SELECT blake3 FROM item
                    WHERE is_checked = true
                    GROUP BY blake3
                    HAVING COUNT(*) > ?)
            GROUP BY item.id
            HAVING COUNT(DISTINCT tag.id) = ?
            ORDER BY item.updated_at DESC LIMIT ? OFFSET ?
            "#,
            serde_json::json!(tags),
            search,
            search,
            limit_dup_count,
            tags.len() as i64,
            limit,
            offset
        )
        .fetch_all(pool)
        .await?;

        let tags_count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM (SELECT item.id FROM item
                LEFT JOIN tag_item ON item.id = tag_item.item
                LEFT JOIN tag ON tag.id = tag_item.tag
                WHERE item.is_checked = true
                    AND tag.name IN (SELECT value FROM json_each(?))
                    AND NOT(item.name COLLATE NOCASE LIKE '%' || ? || '%'
                            OR item.model_name COLLATE NOCASE LIKE '%' || ? || '%')
                    AND blake3 IN (
                        SELECT blake3 FROM item
                        WHERE is_checked = true
                        GROUP BY blake3
                        HAVING COUNT(*) > ?)
                GROUP BY item.id
                HAVING COUNT(DISTINCT tag.id) = ?)"#,
            serde_json::json!(tags),
            search,
            search,
            limit_dup_count,
            tags.len() as i64,
        )
        .fetch_one(pool)
        .await?;

        count += tags_count;
        items.extend(search_by_tags);
    }

    Ok((items.into_iter().collect(), count))
}

pub async fn get_by_hash(pool: &SqlitePool, blake3: &str) -> Result<Item, sqlx::Error> {
    sqlx::query_as!(
        Item,
        "SELECT id, name, path, base_label, note FROM item WHERE is_checked = true AND blake3 = ?",
        blake3
    )
    .fetch_one(pool)
    .await
}
