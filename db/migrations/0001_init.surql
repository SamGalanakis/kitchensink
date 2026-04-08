DEFINE TABLE IF NOT EXISTS auth_user SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS username ON TABLE auth_user TYPE string;
DEFINE FIELD IF NOT EXISTS password_hash ON TABLE auth_user TYPE string;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE auth_user TYPE datetime;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE auth_user TYPE datetime;
DEFINE INDEX IF NOT EXISTS auth_user_username_idx ON TABLE auth_user FIELDS username UNIQUE;

DEFINE TABLE IF NOT EXISTS auth_session SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS user_id ON TABLE auth_session TYPE string;
DEFINE FIELD IF NOT EXISTS token ON TABLE auth_session TYPE string;
DEFINE FIELD IF NOT EXISTS expires_at ON TABLE auth_session TYPE datetime;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE auth_session TYPE datetime;
DEFINE FIELD IF NOT EXISTS last_seen_at ON TABLE auth_session TYPE datetime;
DEFINE INDEX IF NOT EXISTS auth_session_token_idx ON TABLE auth_session FIELDS token UNIQUE;

DEFINE TABLE IF NOT EXISTS app_setting SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS base_url ON TABLE app_setting TYPE string;
DEFINE FIELD IF NOT EXISTS api_key ON TABLE app_setting TYPE option<string>;
DEFINE FIELD IF NOT EXISTS model ON TABLE app_setting TYPE string;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE app_setting TYPE datetime;

DEFINE TABLE IF NOT EXISTS asset SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS kind ON TABLE asset TYPE string ASSERT $value INSIDE ['document', 'image', 'url'];
DEFINE FIELD IF NOT EXISTS source_kind ON TABLE asset TYPE string ASSERT $value INSIDE ['upload', 'url'];
DEFINE FIELD IF NOT EXISTS label ON TABLE asset TYPE string;
DEFINE FIELD IF NOT EXISTS filename ON TABLE asset TYPE option<string>;
DEFINE FIELD IF NOT EXISTS source_url ON TABLE asset TYPE option<string>;
DEFINE FIELD IF NOT EXISTS content_type ON TABLE asset TYPE string;
DEFINE FIELD IF NOT EXISTS byte_size ON TABLE asset TYPE int;
DEFINE FIELD IF NOT EXISTS storage_key ON TABLE asset TYPE string;
DEFINE FIELD IF NOT EXISTS extraction_status ON TABLE asset TYPE string;
DEFINE FIELD IF NOT EXISTS extracted_text ON TABLE asset TYPE option<string>;
DEFINE FIELD IF NOT EXISTS image_description ON TABLE asset TYPE option<string>;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE asset TYPE object;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE asset TYPE datetime;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE asset TYPE datetime;
DEFINE INDEX IF NOT EXISTS asset_source_url_idx ON TABLE asset FIELDS source_url;

DEFINE TABLE IF NOT EXISTS import_job SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS asset_id ON TABLE import_job TYPE string;
DEFINE FIELD IF NOT EXISTS node_id ON TABLE import_job TYPE string;
DEFINE FIELD IF NOT EXISTS headline ON TABLE import_job TYPE string;
DEFINE FIELD IF NOT EXISTS detail ON TABLE import_job TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status ON TABLE import_job TYPE string;
DEFINE FIELD IF NOT EXISTS error ON TABLE import_job TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE import_job TYPE datetime;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE import_job TYPE datetime;

DEFINE TABLE IF NOT EXISTS chat_message SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS role ON TABLE chat_message TYPE string;
DEFINE FIELD IF NOT EXISTS content ON TABLE chat_message TYPE string;
DEFINE FIELD IF NOT EXISTS tool_name ON TABLE chat_message TYPE option<string>;
DEFINE FIELD IF NOT EXISTS meta ON TABLE chat_message TYPE object;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE chat_message TYPE datetime;

DEFINE TABLE IF NOT EXISTS agent_session SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS state ON TABLE agent_session TYPE object;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE agent_session TYPE datetime;

DEFINE TABLE IF NOT EXISTS kg_node SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS kind ON TABLE kg_node TYPE string ASSERT $value INSIDE ['document', 'image', 'url', 'topic'];
DEFINE FIELD IF NOT EXISTS node_id ON TABLE kg_node TYPE string;
DEFINE FIELD IF NOT EXISTS label ON TABLE kg_node TYPE string;
DEFINE FIELD IF NOT EXISTS summary ON TABLE kg_node TYPE option<string>;
DEFINE FIELD IF NOT EXISTS content ON TABLE kg_node TYPE option<string>;
DEFINE FIELD IF NOT EXISTS search_text ON TABLE kg_node TYPE option<string>;
DEFINE FIELD IF NOT EXISTS source ON TABLE kg_node TYPE option<string>;
DEFINE FIELD IF NOT EXISTS asset_id ON TABLE kg_node TYPE option<record<asset>>;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE kg_node TYPE object;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE kg_node TYPE datetime;
DEFINE FIELD IF NOT EXISTS updated_at ON TABLE kg_node TYPE datetime;
DEFINE INDEX IF NOT EXISTS kg_node_kind_node_id_idx ON TABLE kg_node FIELDS kind, node_id UNIQUE;

DEFINE TABLE IF NOT EXISTS kg_edge TYPE RELATION SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS relation ON TABLE kg_edge TYPE string;
DEFINE FIELD IF NOT EXISTS metadata ON TABLE kg_edge TYPE object;
DEFINE FIELD IF NOT EXISTS created_at ON TABLE kg_edge TYPE datetime;

DEFINE ANALYZER IF NOT EXISTS graph_text TOKENIZERS class,camel,punct FILTERS lowercase,ascii;
DEFINE INDEX IF NOT EXISTS kg_node_label_fts ON TABLE kg_node FIELDS label FULLTEXT ANALYZER graph_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS kg_node_summary_fts ON TABLE kg_node FIELDS summary FULLTEXT ANALYZER graph_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS kg_node_content_fts ON TABLE kg_node FIELDS content FULLTEXT ANALYZER graph_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS kg_node_search_text_fts ON TABLE kg_node FIELDS search_text FULLTEXT ANALYZER graph_text BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS kg_node_source_fts ON TABLE kg_node FIELDS source FULLTEXT ANALYZER graph_text BM25 HIGHLIGHTS;
