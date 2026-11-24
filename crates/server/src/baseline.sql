-- The store's schema at 1.0.0, whole (docs/store.md). Each table lists its columns in the order a store at the
-- last release before 1.0.0 holds them.

create table categories (
  name text primary key, purpose text not null, shape text not null, cadence_minutes integer not null,
  gather_minutes integer, provider text, model text, state text not null default 'running', state_at integer,
  created_at integer not null, instruction text, calendar_realm text, calendar_id text);

create table sources (
  id integer primary key autoincrement, category text not null, kind text not null, address text not null,
  query text, require text not null default '[]', exclude text not null default '[]', every_minutes integer,
  item_limit integer not null, failure_count integer not null default 0, last_error text, read_at integer,
  created_at integer not null, changed_at integer not null, scan_count integer not null default 0,
  last_yield integer, last_taken integer, pattern text, strategy text);

create table seen (
  key text primary key, category text not null, name text not null, maker text, address text,
  kind text not null, published_at integer, source_id integer, state text not null, item_id integer,
  found_at integer not null);

create index seen_unqueued on seen (category, state, found_at);

create table queue (
  id integer primary key autoincrement, category text not null, key text not null, name text not null,
  maker text, address text, kind text not null, shape text not null, source_id integer, asked integer not null,
  priority integer not null, state text not null, tries integer not null default 0, retry_at integer,
  run_id integer, document_id integer, title text, description text, share_url text,
  fit integer not null default 0, touches text, plan text,
  job_item_id integer, error text, found_at integer not null, created_at integer not null,
  changed_at integer not null, origin text not null default 'source', because text, parent_item_id integer,
  parent_document_id integer, parent_title text, parent_share_url text, repair_item_id integer,
  error_at integer);

create index queue_state on queue (category, state);

create table runs (
  id integer primary key autoincrement, category text not null, item_id integer not null,
  item_ids text not null, try integer not null, run_id integer, state text not null, outcome text, error text,
  started_at integer not null, ended_at integer);

create table reports (
  id integer primary key autoincrement, item_id integer not null, recipient text not null, kind text not null,
  instruction_id integer, state text not null, error text, at integer not null);

create table recipients (
  id integer primary key autoincrement, category text not null, kind text not null, room_id integer,
  agent_name text, origin text not null, added_at integer not null);

create unique index recipients_one on recipients (category, kind, coalesce(room_id, 0), coalesce(agent_name, ''));

create table facts (
  id integer primary key autoincrement, name text not null, value text not null, unit text, as_of text,
  source_address text, run_id integer, document_id integer, created_at integer not null);

create index facts_name on facts (name, id);

create table stack (
  id integer primary key autoincrement, component text not null, version text, place text, repository text,
  worker text, added_at integer not null);

create table research_runs (
  id integer primary key autoincrement, key text not null, line text not null, query text not null,
  asker text, options text, room_id integer, provider text not null, model text not null,
  state text not null, outcome text, error text, started_at integer not null, finished_at integer,
  branches_allowed integer not null, branches integer not null default 0,
  record text not null, tells integer not null, turns text not null default '[]', written text,
  document_id integer, title text, description text, link text, continues integer,
  answered text not null default '[]', review_asks integer not null default 0);

create index research_runs_state on research_runs (state);

create table research_log (
  id integer primary key autoincrement, run_id integer not null, at integer not null, kind text not null,
  text text not null);

create index research_log_run on research_log (run_id, id);

create table research_holds (
  id integer primary key autoincrement, run_id integer not null, kind text not null, value text not null);

create index research_holds_run on research_holds (run_id);

create table research_settings (id integer primary key, value text not null);

create table renderings (
  document_id integer primary key, title text not null, description text, body text not null,
  created_at integer not null);

create table demands (
  id integer primary key, try_id integer, run_id integer, within integer, service text not null,
  asked_at integer not null);

create table syntheses (
  document_id integer not null, language text not null, synthesis_id integer not null,
  characters integer not null, asked_at integer not null, primary key (document_id, language));

create table translated_parts (
  document_id integer not null, english text not null, korean text not null,
  primary key (document_id, english));

create table page_texts (
  document_id integer not null, name text not null, text text not null, primary key (document_id, name));

create table page_reviews (
  document_id integer primary key, answer text not null, applied text not null default '[]',
  remade text not null default '[]', reviewed_at integer not null);

create table narration_asks (
  document_id integer not null, language text not null, renewed integer not null,
  primary key (document_id, language));

create table pages (
  document_id integer primary key, item_id integer, languages text not null default '[]',
  narrations text not null default '[]', pending text not null default '[]', pending_retry_at integer,
  translation_tries integer not null default 0, narration_tries integer not null default 0, gone_at integer);
