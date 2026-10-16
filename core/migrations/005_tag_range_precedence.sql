-- Splitting a range retains its original creation precedence in overlap reports.
ALTER TABLE tag_ranges ADD COLUMN precedence_id INTEGER CHECK (precedence_id > 0);
