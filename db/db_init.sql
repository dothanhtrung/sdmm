create table app_info
(
    label TEXT    not null,
    value integer not null
);

create table item
(
    id         integer              not null
        constraint item_pk
            primary key autoincrement,
    path       TEXT                 not null,
    base_label TEXT                 not null,
    blake3     TEXT    default ''   not null,
    is_checked integer default true not null,
    name       TEXT    default ''   not null,
    note       TEXT    default ''   not null,
    created_at INTEGER,
    updated_at integer,
    model_name TEXT    default ''   not null,
    constraint item_pk_2
        unique (path, base_label)
);

create table tag_type
(
    id   integer not null
        constraint tag_type_pk
            primary key autoincrement,
    type TEXT    not null
);

create table tag
(
    name        TEXT            not null
        constraint tag_pk_2
            unique,
    description TEXT default '' not null,
    id          integer         not null
        constraint tag_pk
            primary key autoincrement,
    type        integer
        constraint tag_tag_type_id_fk
            references tag_type
);

create table tag_item
(
    tag  INTEGER not null
        constraint tag_item_tag_id_fk
            references tag
            on update cascade on delete cascade,
    item integer not null
        constraint tag_model_model_id_fk
            references item
            on update cascade on delete cascade,
    constraint tag_item_pk
        primary key (tag, item)
);

create unique index tag_item_item_tag_uindex
    on tag_item (item, tag);

create table tag_tag
(
    tag INTEGER not null
        constraint tag_tag_tag_id_fk
            references tag
            on update cascade on delete cascade,
    dep INTEGER not null
        constraint tag_tag_tag_id_fk_2
            references tag
            on update cascade on delete cascade,
    constraint tag_tag_pk
        primary key (tag, dep)
);


