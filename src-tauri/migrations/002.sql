BEGIN IMMEDIATE;
ALTER TABLE profiles ADD COLUMN name TEXT NOT NULL DEFAULT 'My workspace';
UPDATE profiles SET name='Workspace ' || rowid;
CREATE UNIQUE INDEX one_active_workspace ON profiles(active) WHERE active=1;
PRAGMA user_version=2;
COMMIT;
