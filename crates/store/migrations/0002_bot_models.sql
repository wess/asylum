-- A Bot may use its own provider profile and model; empty means the default.
ALTER TABLE bots ADD COLUMN provider TEXT NOT NULL DEFAULT '';
ALTER TABLE bots ADD COLUMN model TEXT NOT NULL DEFAULT '';
