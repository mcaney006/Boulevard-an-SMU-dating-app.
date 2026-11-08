create table if not exists boulevard_profiles (
  user_id text primary key,
  display_name text not null,
  year_label text not null,
  major text not null,
  greek text not null default '',
  intent text not null,
  looking_for text not null,
  bio text not null,
  updated_at timestamptz not null default now()
);

create table if not exists boulevard_swipes (
  user_id text not null,
  person_id text not null,
  direction text not null,
  created_at timestamptz not null default now(),
  primary key (user_id, person_id)
);

create table if not exists boulevard_messages (
  id serial primary key,
  user_id text not null,
  person_id text not null,
  body text not null,
  from_me boolean not null,
  created_at timestamptz not null default now()
);

create index if not exists boulevard_messages_user_person_idx
  on boulevard_messages (user_id, person_id, id);

create table if not exists boulevard_plus (
  user_id text primary key,
  active boolean not null default false,
  updated_at timestamptz not null default now()
);
