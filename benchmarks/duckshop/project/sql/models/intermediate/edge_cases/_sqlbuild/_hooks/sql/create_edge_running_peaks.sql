HOOK (
  description """
    Create the edge-case JavaScript function edge_running_peaks, which turns a list of numbers into
    the running maximum at each position. SQLBuild does not declare functions a hook creates.
  """,
);

CREATE OR REPLACE FUNCTION @@CTX:destination.database.@@CTX:destination.schema.edge_running_peaks(
  line_numbers ARRAY
)
RETURNS ARRAY
LANGUAGE JAVASCRIPT
AS $$
  var peaks = [];
  var peak = null;
  for (var i = 0; i < LINE_NUMBERS.length; i++) {
    var value = LINE_NUMBERS[i];
    if (value !== null && (peak === null || value > peak)) {
      peak = value;
    }
    peaks.push(peak);
  }
  return peaks;
$$
