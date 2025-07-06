//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use sqlx::sqlite::SqliteQueryResult;
use sqlx::SqlitePool;
use std::collections::HashSet;

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
    };
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
    model_name: &str,
) -> Result<i64, sqlx::Error> {
    let mut ret_id = 0;

    if let Ok(id) = sqlx::query_scalar!(
        r#"SELECT id FROM item WHERE path = ? AND base_label = ?"#,
        path,
        base_label
    )
    .fetch_one(pool)
    .await
    {
        sqlx::query!(r#"UPDATE item SET is_checked=true WHERE id = ?"#, id,)
            .execute(pool)
            .await?;
        ret_id = id;
    } else {
        ret_id = sqlx::query!(
            r#"INSERT INTO item (name, model_name, path, base_label, blake3) VALUES (?, ?, ?, ?, ?) "#,
            name,
            model_name,
            path,
            base_label,
            blake3,
        )
        .execute(pool)
        .await?
        .last_insert_rowid();
    }

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

pub async fn get(pool: &SqlitePool, limit: i64, offset: i64) -> Result<(Vec<Item>, i64), sqlx::Error> {
    let items = sqlx::query_as!(
        Item,
        r#"SELECT id, name, path, base_label, note FROM item WHERE is_checked = true ORDER BY id DESC LIMIT ? OFFSET ?"#,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    let total = sqlx::query_scalar!("SELECT count(id) FROM item WHERE is_checked = true",)
        .fetch_one(pool)
        .await?;

    Ok((items, total))
}

pub async fn get_tags(pool: &SqlitePool, id: i64) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar!(
        "SELECT tag.name FROM tag LEFT JOIN tag_item ON tag.id = tag_item.tag WHERE tag_item.item = ? ORDER BY tag.name",
        id
    )
    .fetch_all(pool)
    .await
}

pub async fn search(
    pool: &SqlitePool,
    search: &str,
    limit: i64,
    offset: i64,
    tag_only: bool,
) -> Result<(Vec<Item>, i64), sqlx::Error> {
    let mut items = HashSet::new();
    let mut count = 0;
    if !tag_only {
        let  items_by_name = sqlx::query_as!(
        Item,
        r#"SELECT id,name, path, base_label, note FROM item WHERE is_checked = true AND name COLLATE NOCASE LIKE '%' || ? || '%' OR model_name LIKE '%' || ? || '%' ORDER BY id DESC LIMIT ? OFFSET ?"#,
        search, search, limit, offset
    )
            .fetch_all(pool)
            .await?;
        let count_by_name = sqlx::query_scalar!(
            "SELECT count(id) FROM item WHERE is_checked = true AND name LIKE '%' || ? || '%' OR model_name LIKE '%' || ? || '%'",
            search,
            search
        )
        .fetch_one(pool)
        .await?;

        items.extend(items_by_name);
        count += count_by_name;
    }

    let tags: Vec<String> = search.split_whitespace().map(|s| s.to_string()).collect();

    if !tags.is_empty() {
        let condition = format!(
            "FROM item
          LEFT JOIN tag_item ON item.id = tag_item.item
          LEFT JOIN tag ON tag.id = tag_item.tag
          WHERE item.is_checked = true
            AND tag.name IN ('{}')",
            tags.join("','")
        );
        let query = format!(
            "SELECT item.id as id, item.name as name, item.note as note, item.path as path, item.base_label as base_label {} ORDER BY item.id DESC LIMIT {} OFFSET {}",
            condition,
            limit,
            offset
        );
        let search_by_tags: Vec<Item> = sqlx::query_as(&query).fetch_all(pool).await?;

        let count_query = format!("SELECT count(*) {} ", condition);
        let tags_count: i64 = sqlx::query_scalar(&count_query).fetch_one(pool).await?;

        count += tags_count;
        items.extend(search_by_tags);
    }

    Ok((items.into_iter().collect(), count))
}
