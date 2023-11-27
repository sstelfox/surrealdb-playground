-- vim: sw=4 ts=4 expandtab ai ft=sql

-- Create our default namespace and make sure its active
DEFINE NAMESPACE generative_ontology;
USE NS generative_ontology;

-- Create a database storing central configuration and system properties
DEFINE DATABASE core;
USE DB core;

-- We'll act in different roles from within our REPL and those roles should be
-- restricted to their intended behavior
DEFINE TABLE role SCHEMAFULL
    PERMISSIONS
        FOR SELECT, UPDATE WHERE id = $auth.id,
        FOR CREATE, DELETE NONE;

DEFINE FIELD name ON role TYPE string ASSERT string::len($value) > 3;
DEFINE FIELD description ON role TYPE string;

DEFINE INDEX idx_role_name ON role COLUMNS name UNIQUE;

-- Create our initial roles
CREATE role:uuid() SET
    name = "generator",
    description = "The actor that performs searches against the database and creates concrete matching graphs that satisfy the constraints of the ontology.";
CREATE role:uuid() SET
    name = "ontologist",
    description = "Effectively defines the rules of the generator to follow by constraining processes, providing alternatives, and making associations in the internal ontology.";

-- We do not allow sign ups, roles must be defined by a non-scoped user
DEFINE SCOPE role
    SIGNIN (
        SELECT * FROM role WHERE name = $name
    );
