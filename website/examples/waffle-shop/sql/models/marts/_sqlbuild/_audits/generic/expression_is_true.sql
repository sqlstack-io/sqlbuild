AUDIT ();

SELECT *
FROM __ref("@model")
WHERE NOT (@expression)
