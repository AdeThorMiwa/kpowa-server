-- Add migration script here
alter table users add created_on timestamptz not null default now();