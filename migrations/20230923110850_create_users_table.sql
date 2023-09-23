-- Add migration script here
create table users (
    uid uuid primary key,
    username varchar(255) unique not null,
    invite_code varchar(255) unique not null,
    referred_by varchar(255)
);