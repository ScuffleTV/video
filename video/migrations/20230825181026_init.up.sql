--- Types & Tables

CREATE TYPE rendition AS ENUM ('VIDEO_SOURCE', 'VIDEO_HD', 'VIDEO_SD', 'VIDEO_LD', 'AUDIO_SOURCE');
CREATE TYPE playback_session_device AS ENUM ('UNKNOWN');
CREATE TYPE playback_session_platform AS ENUM ('UNKNOWN');
CREATE TYPE playback_session_browser AS ENUM ('UNKNOWN');
CREATE TYPE room_status AS ENUM ('OFFLINE', 'WAITING_FOR_TRANSCODER', 'READY');
CREATE TYPE visibility AS ENUM ('PUBLIC', 'PRIVATE');

-- The top level organization object, this is the root of the tree.
-- All other objects are children of this object.
CREATE TABLE organizations (
    id UUID NOT NULL PRIMARY KEY,

    name VARCHAR(32) NOT NULL,
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),

    tags JSONB NOT NULL DEFAULT '{}'::JSONB
);

-- Access tokens are used to authenticate with the API.
-- They are scoped to an organization and have a set of permissions.
CREATE TABLE access_tokens (
    organization_id UUID NOT NULL,
    id UUID NOT NULL,

    secret_token UUID NOT NULL,
    scopes bytes[] NOT NULL,
    last_active_at TIMESTAMPTZ(3),
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ(3),

    tags JSONB NOT NULL DEFAULT '{}'::JSONB,

    PRIMARY KEY (organization_id, id)
);

-- S3 buckets are used to store recordings and thumbnails.
-- They are scoped to an organization.
CREATE TABLE s3_buckets (
    organization_id UUID NOT NULL,
    id UUID NOT NULL,

    name VARCHAR(64) NOT NULL,
    region VARCHAR(64) NOT NULL,
    endpoint VARCHAR(256),
    access_key_id VARCHAR(256) NOT NULL,
    secret_access_key VARCHAR(256) NOT NULL,
    public_url VARCHAR(256),
    managed BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),

    tags JSONB NOT NULL DEFAULT '{}'::JSONB,

    PRIMARY KEY (organization_id, id)
);

-- Transcoding configs are used to configure the transcoding pipeline.
-- They are scoped to an organization.
CREATE TABLE transcoding_configs (
    organization_id UUID NOT NULL,
    id UUID NOT NULL,

    renditions rendition[] NOT NULL DEFAULT ARRAY['VIDEO_SOURCE', 'AUDIO_SOURCE'],
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),

    tags JSONB NOT NULL DEFAULT '{}'::JSONB,

    PRIMARY KEY (organization_id, id)
);

-- Recording configs are used to configure the recording pipeline.
-- They are scoped to an organization and contain a reference to the s3 bucket that recordings should be stored in.
CREATE TABLE recording_configs (
    organization_id UUID NOT NULL,
    id UUID NOT NULL,

    renditions rendition[] NOT NULL DEFAULT ARRAY['VIDEO_SOURCE', 'VIDEO_HD', 'VIDEO_SD', 'VIDEO_LD', 'AUDIO_SOURCE'],
    lifecycle_policies bytes[] NOT NULL DEFAULT ARRAY[],
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),
    s3_bucket_id UUID NOT NULL,

    tags JSONB NOT NULL DEFAULT '{}'::JSONB,

    PRIMARY KEY (organization_id, id)
);

-- Rooms are used to configure the live streaming pipeline.
-- They are scoped to an organization and contain a reference to the transcoding config and recording config that should be used.
-- They also have a bunch of auxiliary fields that are used to track the state of the room.
CREATE TABLE rooms (
    organization_id UUID NOT NULL,
    id UUID NOT NULL,

    transcoding_config_id UUID,
    recording_config_id UUID,

    visibility visibility NOT NULL DEFAULT 'PUBLIC',

    stream_key CHAR(32) NOT NULL,
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),

    last_live_at TIMESTAMPTZ(3),
    last_disconnected_at TIMESTAMPTZ(3),

    tags JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- Room Session Stuff 

    status room_status NOT NULL DEFAULT 'OFFLINE',

    video_input bytes,
    audio_input bytes,

    active_ingest_connection_id UUID,
    active_recording_config bytes,
    active_transcoding_config bytes,
    active_recording_id UUID,

    ingest_bitrate INT,

    video_output bytes[],
    audio_output bytes[],

    PRIMARY KEY (organization_id, id)
);

-- Recordings are used to track the state of a recording.
-- They are scoped to an organization and contain a reference to the room they were recorded in, the recording config that was used, and the s3 bucket that they are stored in.
CREATE TABLE recordings (
    id UUID NOT NULL,
    organization_id UUID NOT NULL,

    room_id UUID,
    recording_config_id UUID,
    s3_bucket_id UUID NOT NULL,

    visibility visibility NOT NULL DEFAULT 'PUBLIC',
    allow_dvr BOOLEAN NOT NULL DEFAULT FALSE,

    deleted_at TIMESTAMPTZ(3),
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),
    ended_at TIMESTAMPTZ(3),

    tags JSONB NOT NULL DEFAULT '{}'::JSONB,

    PRIMARY KEY (organization_id, id)
);

-- Recording renditions are used to track the state of a recording.
-- Which renditions are saved and what the configuration for the rendition is is stored here.
CREATE TABLE recording_renditions (
    organization_id UUID NOT NULL,
    recording_id UUID NOT NULL,
    rendition rendition NOT NULL,
    config bytes NOT NULL,

    PRIMARY KEY (organization_id, recording_id, rendition)
);

-- Recording thumbnails are images that are generated from the recording.
-- They are used to display a preview of the recording, and are stored in the s3 bucket that the recording is stored in.
-- They are scoped to an organization and contain a reference to the recording they were generated from.
CREATE TABLE recording_thumbnails (
    organization_id UUID NOT NULL,
    recording_id UUID NOT NULL,
    idx INT4 NOT NULL,
    id UUID NOT NULL,
    start_time REAL NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,

    PRIMARY KEY (organization_id, recording_id, idx)
);

-- Recording rendition segments are the individual segments that make up a recording rendition.
-- They are resources that are stored in the s3 bucket defined in the recording.
-- They are scoped to an organization and contain a reference to the recording.
CREATE TABLE recording_rendition_segments (
    organization_id UUID NOT NULL,
    recording_id UUID NOT NULL,
    rendition rendition NOT NULL,
    idx INT4 NOT NULL,
    id UUID NOT NULL,
    start_time REAL NOT NULL,
    end_time REAL NOT NULL,
    size_bytes INT4 NOT NULL DEFAULT 0,

    PRIMARY KEY (organization_id, recording_id, rendition, idx)
);

-- Playback key pairs are used to sign playback sessions.
-- They are scoped to an organization.
CREATE TABLE playback_key_pairs (
    organization_id UUID NOT NULL,
    id UUID NOT NULL,
    public_key bytes NOT NULL,
    fingerprint VARCHAR(64) NOT NULL,
    updated_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW(),
    tags JSONB NOT NULL DEFAULT '{}'::JSONB,

    PRIMARY KEY (organization_id, id)
);

-- Playback sessions are sessions watching a recording or room.
-- They are scoped to an organization and contain a reference to the playback key pair that was used to sign them (if any) or are anonymous.
-- They also contain a reference to the room or recording that they are watching.
-- This table is special since there will be a lot of rows in it, so we use a TTL to automatically delete old rows.
CREATE TABLE playback_sessions (
    organization_id UUID NOT NULL,
    id UUID NOT NULL,

    room_id UUID,
    recording_id UUID,
    user_id VARCHAR(128),
    ip_address INET NOT NULL,

    playback_key_pair_id UUID,

    issued_at TIMESTAMPTZ(3),
    expires_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW() + INTERVAL '10 minutes',
    user_agent VARCHAR(256),
    referer VARCHAR(256),
    origin VARCHAR(256),
    device playback_session_device NOT NULL DEFAULT 'UNKNOWN',
    platform playback_session_platform NOT NULL DEFAULT 'UNKNOWN',
    browser playback_session_browser NOT NULL DEFAULT 'UNKNOWN',
    player_version VARCHAR(32),

    PRIMARY KEY (organization_id, id)
) WITH (ttl_expiration_expression = 'expires_at');

-- Session token revokes are used to revoke playback sessions.
-- They are scoped to an organization and contain a reference to the playback session that they are revoking.
-- This table isnt exposed to the end user, but is used internally to prevent revoked sessions from being authenticated.
CREATE TABLE playback_session_revocations (
    organization_id UUID NOT NULL,
    room_id UUID DEFAULT NULL,
    recording_id UUID DEFAULT NULL,
    user_id VARCHAR(128) DEFAULT NULL,
    sso_id VARCHAR(128) DEFAULT NULL,

    expires_at TIMESTAMPTZ(3) NOT NULL DEFAULT NOW() + INTERVAL '10 minutes',
    revoke_before TIMESTAMPTZ(3) NOT NULL DEFAULT NOW()
) WITH (ttl_expiration_expression = 'expires_at');

--- Relationships & Indexes
-- All fk's on organizations cascade because we want to delete all objects when an organization is deleted.
-- Typically fk's are not cascade because we should clean up children objects before deleting the parent.

-- Access Tokens
ALTER TABLE access_tokens ADD CONSTRAINT access_tokens_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
CREATE INVERTED INDEX idx_access_tokens_tags ON access_tokens(organization_id, tags);

-- S3 Buckets
ALTER TABLE s3_buckets ADD CONSTRAINT s3_buckets_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
CREATE INVERTED INDEX idx_s3_buckets_tags ON s3_buckets(organization_id, tags);

-- Transcoding Configs
ALTER TABLE transcoding_configs ADD CONSTRAINT transcoding_configs_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
CREATE INVERTED INDEX idx_transcoding_configs_tags ON transcoding_configs(organization_id, tags);

-- Recording Configs
ALTER TABLE recording_configs ADD CONSTRAINT recording_configs_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
CREATE INVERTED INDEX idx_recording_configs_tags ON recording_configs(organization_id, tags);

-- Rooms
ALTER TABLE rooms ADD CONSTRAINT rooms_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
ALTER TABLE rooms ADD CONSTRAINT rooms_transcoding_config_id_fkey FOREIGN KEY (organization_id, transcoding_config_id) REFERENCES transcoding_configs(organization_id, id);
ALTER TABLE rooms ADD CONSTRAINT rooms_recording_config_id_fkey FOREIGN KEY (organization_id, recording_config_id) REFERENCES recording_configs(organization_id, id);
ALTER TABLE rooms ADD CONSTRAINT rooms_active_recording_id_fkey FOREIGN KEY (organization_id, active_recording_id) REFERENCES recordings(organization_id, id);
CREATE INVERTED INDEX idx_rooms_tags ON rooms(organization_id, tags);
CREATE INDEX idx_rooms_status ON rooms(organization_id, status);
CREATE INDEX idx_rooms_visibility ON rooms(organization_id, visibility);
CREATE INDEX idx_rooms_transcoding_config_id ON rooms(organization_id, transcoding_config_id);
CREATE INDEX idx_rooms_recording_config_id ON rooms(organization_id, recording_config_id);

-- Recordings
ALTER TABLE recordings ADD CONSTRAINT recordings_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
ALTER TABLE recordings ADD CONSTRAINT recordings_room_id_fkey FOREIGN KEY (organization_id, room_id) REFERENCES rooms(organization_id, id);
ALTER TABLE recordings ADD CONSTRAINT recordings_recording_config_id_fkey FOREIGN KEY (organization_id, recording_config_id) REFERENCES recording_configs(organization_id, id);
ALTER TABLE recordings ADD CONSTRAINT recordings_s3_bucket_id_fkey FOREIGN KEY (organization_id, s3_bucket_id) REFERENCES s3_buckets(organization_id, id);
CREATE INVERTED INDEX idx_recordings_tags ON recordings(organization_id, tags);
CREATE INDEX idx_recordings_visibility ON recordings(organization_id, visibility);
CREATE INDEX idx_recordings_room_id ON recordings(organization_id, room_id);
CREATE INDEX idx_recordings_recording_config_id ON recordings(organization_id, recording_config_id);
CREATE INDEX idx_recordings_s3_bucket_id ON recordings(organization_id, s3_bucket_id);

-- Recording Renditions
ALTER TABLE recording_renditions ADD CONSTRAINT recording_renditions_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
ALTER TABLE recording_renditions ADD CONSTRAINT recording_renditions_recording_id_fkey FOREIGN KEY (organization_id, recording_id) REFERENCES recordings(organization_id, id);

-- Recording Thumbnails
ALTER TABLE recording_thumbnails ADD CONSTRAINT recording_thumbnails_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
ALTER TABLE recording_thumbnails ADD CONSTRAINT recording_thumbnails_recording_id_fkey FOREIGN KEY (organization_id, recording_id) REFERENCES recordings(organization_id, id);

-- Recording Rendition Segments
ALTER TABLE recording_rendition_segments ADD CONSTRAINT recording_rendition_segments_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;

-- The reason this is bound to recordings and not recording_renditions is because we want to be able to delete a recording rendition without deleting the segments, this is because
-- The segments require resources to be cleaned up outside of the database, and we don't want to have to do that when we delete a recording rendition.
-- We can schedule a job to clean up the segments later.
ALTER TABLE recording_rendition_segments ADD CONSTRAINT recording_rendition_segments_recording_id_fkey FOREIGN KEY (organization_id, recording_id) REFERENCES recordings(organization_id, id);

-- Playback Key Pairs
ALTER TABLE playback_key_pairs ADD CONSTRAINT playback_key_pairs_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
CREATE INVERTED INDEX idx_playback_key_pairs_tags ON playback_key_pairs(organization_id, tags);

-- Playback Sessions
-- PlaybackSessions are a special object because we do want to purge the parents without deleting the children, which is why they are delete cascade.
ALTER TABLE playback_sessions ADD CONSTRAINT playback_sessions_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
ALTER TABLE playback_sessions ADD CONSTRAINT playback_sessions_room_id_fkey FOREIGN KEY (organization_id, room_id) REFERENCES rooms(organization_id, id) ON DELETE CASCADE;
ALTER TABLE playback_sessions ADD CONSTRAINT playback_sessions_recording_id_fkey FOREIGN KEY (organization_id, recording_id) REFERENCES recordings(organization_id, id) ON DELETE CASCADE;
ALTER TABLE playback_sessions ADD CONSTRAINT playback_sessions_playback_key_pair_id_fkey FOREIGN KEY (organization_id, playback_key_pair_id) REFERENCES playback_key_pairs(organization_id, id) ON DELETE CASCADE;
CREATE INDEX idx_playback_sessions_room_id_recording_id_user_id_ip_address ON playback_sessions(organization_id, room_id, recording_id, user_id, ip_address);
CREATE INDEX idx_playback_sessions_user_id_room_id_recording_id ON playback_sessions(organization_id, user_id, room_id, recording_id);
CREATE INDEX idx_playback_sessions_expires_at ON playback_sessions(expires_at);

-- Session Token Revokes
-- SessionTokenRevokes are a special object because we do want to purge the parents without deleting the children, which is why they are delete cascade.
ALTER TABLE playback_session_revocations ADD CONSTRAINT playback_session_revocations_organization_id_fkey FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE;
ALTER TABLE playback_session_revocations ADD CONSTRAINT playback_session_revocations_room_id_fkey FOREIGN KEY (organization_id, room_id) REFERENCES rooms(organization_id, id) ON DELETE CASCADE;
ALTER TABLE playback_session_revocations ADD CONSTRAINT playback_session_revocations_recording_id_fkey FOREIGN KEY (organization_id, recording_id) REFERENCES recordings(organization_id, id) ON DELETE CASCADE;
CREATE INDEX idx_playback_session_revocations_room_id_recording_id_user_id_sso_id ON playback_session_revocations(organization_id, room_id, recording_id, user_id, sso_id);
CREATE INDEX idx_playback_session_revocations_sso_id ON playback_session_revocations(organization_id, sso_id);
ALTER TABLE playback_session_revocations ADD CONSTRAINT playback_session_revocations UNIQUE (organization_id, sso_id);
CREATE INDEX idx_playback_session_revocations_expires_at ON playback_session_revocations(expires_at);
