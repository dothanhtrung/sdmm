
create table if not exists preview
(
    id              integer not null
        constraint preview_pk
            primary key autoincrement,
    path            TEXT    not null
        constraint preview_pk_2
            unique,
    base_label      TEXT    not null,
    blake3          TEXT    not null,
    positive_prompt TEXT    not null,
    negative_prompt TEXT    not null,
    cfg             integer not null,
    step            integer not null,
    sampler         TEXT    not null,
    clip_skip       integer not null,
    width           integer not null,
    height          integer not null
);

create table if not exists preview_item
(
    id      integer not null
        constraint preview_item_pk
            primary key autoincrement,
    preview integer not null
        constraint preview_item_preview_id_fk
            references preview
            on delete cascade,
    item    integer not null
        constraint preview_item_item_id_fk
            references item
            on delete cascade,
    constraint preview_item_pk_2
        unique (item, preview)
);
