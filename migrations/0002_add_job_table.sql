create table job
(
    id         integer                           not null
        constraint job_pk
            primary key autoincrement,
    title      text                              not null,
    desc       text                              not null,
    state      integer                           not null,
    started_at integer default current_timestamp not null,
    stopped_at integer
);


