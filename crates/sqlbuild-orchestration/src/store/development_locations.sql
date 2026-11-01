CREATE TABLE sqb_development_locations (
    project_path TEXT NOT NULL,
    target TEXT NOT NULL,
    location_id {uuid} NOT NULL UNIQUE REFERENCES sqb_locations(location_id),
    PRIMARY KEY (project_path, target)
);
