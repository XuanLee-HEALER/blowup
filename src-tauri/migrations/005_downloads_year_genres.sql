-- Add year and genres columns to downloads table
ALTER TABLE downloads ADD COLUMN year INTEGER;
ALTER TABLE downloads ADD COLUMN genres TEXT DEFAULT '';
