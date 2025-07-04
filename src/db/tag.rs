//! Copyright (c) 2025 Trung Do <dothanhtrung@pm.me>.

use crate::civitai::{CivitaiFileMetadata, CivitaiModel};
use actix_web_lab::__reexports::futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, SqlitePool};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, FromRow)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub deps: Option<String>,
}

pub async fn get_tag_by_name(pool: &SqlitePool, name: &str) -> Result<Tag, sqlx::Error> {
    sqlx::query_as!(Tag, r#"SELECT tmp.id as id, tmp.name as name , tmp.description as description, GROUP_CONCAT(tag.name, ' ') as "deps:_"
FROM
    (SELECT tag.id as id, tag.name as name, tag.description as description, tag_tag.dep as dep FROM tag LEFT JOIN tag_tag ON tag.id = tag_tag.tag) as tmp
LEFT JOIN tag ON tag.id = tmp.dep WHERE tmp.name = ? GROUP BY tmp.id
"#, name).fetch_one(pool).await
}

pub async fn add_tag(pool: &SqlitePool, name: &str) -> anyhow::Result<()> {
    sqlx::query!("INSERT OR IGNORE INTO tag (name) VALUES (?)", name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM tag WHERE id = ?", id).execute(pool).await?;
    Ok(())
}

pub async fn update_tag(pool: &SqlitePool, tag: &Tag) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE tag SET name = ?, description = ? WHERE id = ?",
        tag.name,
        tag.description,
        tag.id
    )
    .execute(pool)
    .await?;

    for dep in tag
        .deps
        .clone()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<&str>>()
    {
        let dep_id = match sqlx::query_scalar!("SELECT id FROM tag WHERE name = ?", dep)
            .fetch_one(pool)
            .await
        {
            Ok(id) => id,
            Err(_) => sqlx::query!("INSERT INTO tag (name) VALUES (?)", dep)
                .execute(pool)
                .await?
                .last_insert_rowid(),
        };

        sqlx::query!("INSERT OR IGNORE INTO tag_tag (tag, dep) VALUES (?, ?)", tag.id, dep_id)
            .execute(pool)
            .await?;
    }

    Ok(())
}

pub async fn add_tag_item(pool: &SqlitePool, item: i64, tags: &Vec<String>) -> Result<(), sqlx::Error> {
    let added_tags = sqlx::query_scalar!("SELECT tag FROM tag_item WHERE item = ?", item)
        .fetch_all(pool)
        .await?;
    let mut exist_tags = HashSet::new();
    exist_tags.extend(added_tags);

    let mut depend_tags = HashSet::new();

    for tag in tags {
        if tag.is_empty() {
            continue;
        }

        let tag_id = match sqlx::query_scalar!("SELECT id FROM tag WHERE name = ?", tag)
            .fetch_one(pool)
            .await
        {
            Ok(id) => id,
            Err(_) => sqlx::query!("INSERT INTO tag (name) VALUES (?)", tag)
                .execute(pool)
                .await?
                .last_insert_rowid(),
        };

        if !exist_tags.contains(&tag_id) {
            sqlx::query!("INSERT OR IGNORE INTO tag_item (item, tag) VALUES (?, ?)", item, tag_id)
                .execute(pool)
                .await?;

            exist_tags.insert(tag_id);

            let deps = sqlx::query_scalar!("SELECT dep FROM tag_tag WHERE tag = ?", tag_id)
                .fetch_all(pool)
                .await?;
            depend_tags.extend(deps);
        }
    }

    let mut tmp_dep_tags = HashSet::new();
    while !depend_tags.is_empty() {
        for tag in depend_tags.iter() {
            if !exist_tags.contains(tag) {
                sqlx::query!("INSERT OR IGNORE INTO tag_item (item, tag) VALUES (?, ?)", item, tag)
                    .execute(pool)
                    .await?;
                exist_tags.insert(*tag);

                let deps = sqlx::query_scalar!("SELECT dep FROM tag_tag WHERE tag = ?", tag)
                    .fetch_all(pool)
                    .await?;
                tmp_dep_tags.extend(deps);
            }
        }
        depend_tags = tmp_dep_tags;
        tmp_dep_tags = HashSet::new();
    }

    Ok(())
}

pub async fn add_tag_from_model_info(
    pool: &SqlitePool,
    item: i64,
    extra_tags: &Vec<String>,
    model_info: &CivitaiModel,
    file_metadata: &CivitaiFileMetadata,
) -> Result<(), sqlx::Error> {
    let mut tags = Vec::new();
    for tag in extra_tags {
        tags.push(tag.clone().replace(" ", "_").to_lowercase());
    }

    tags.push(model_info.model_type.clone().replace(" ", "_").to_lowercase());
    if model_info.nsfw {
        tags.push(String::from("nsfw"));
    }
    if model_info.poi {
        tags.push(String::from("poi"));
    }
    tags.push(file_metadata.format.clone().replace(" ", "_").to_lowercase());
    if let Some(fp) = &file_metadata.fp {
        tags.push(fp.to_string());
    }
    if let Some(size) = &file_metadata.size {
        tags.push(size.to_string());
    }
    add_tag_item(pool, item, &tags).await
}

pub async fn update_tag_item(pool: &SqlitePool, item: i64, tag_str: &str) -> anyhow::Result<()> {
    sqlx::query!("DELETE FROM tag_item WHERE item = ?", item)
        .execute(pool)
        .await?;

    let mut tags = Vec::new();
    for tag in tag_str.split_whitespace() {
        tags.push(tag.to_lowercase());
    }
    add_tag_item(pool, item, &tags).await?;
    Ok(())
}

pub async fn update_item_note(pool: &SqlitePool, item: i64, note: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("UPDATE item SET note = ? WHERE id = ?", note, item)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn list_tags(pool: &SqlitePool, item_ids: HashSet<i64>) -> Result<Vec<TagCount>, sqlx::Error> {
    if item_ids.is_empty() {
        sqlx::query_as!(
            TagCount,
            r#"SELECT tag.name as tag, COUNT(*) as count FROM tag
                LEFT JOIN tag_item ON tag.id = tag_item.tag
                LEFT JOIN item ON item.id = tag_item.item
                WHERE item.is_checked = true
                GROUP BY tag_item.tag ORDER BY count DESC"#
        )
        .fetch_all(pool)
        .await
    } else {
        let placeholders = vec!["?"; item_ids.len()].join(",");
        let sql = format!(
            "SELECT tag.name as tag, COUNT(*) as count FROM tag LEFT JOIN tag_item ON tag.id = tag_item.tag \
        LEFT JOIN item ON item.id = tag_item.item \
        WHERE item.is_checked = true AND tag_item.item IN ({placeholders}) GROUP BY tag_item.tag ORDER BY count DESC"
        );
        let mut query = sqlx::query_as::<_, TagCount>(&sql);
        for id in item_ids {
            query = query.bind(id);
        }
        query.fetch_all(pool).await
    }
}
