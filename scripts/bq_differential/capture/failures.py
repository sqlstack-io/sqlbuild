"""Write the analysis failure corpus to the given directory, each project rewired to BigQuery."""
import re, shutil, sys
from pathlib import Path
from scripts.compiler_differential._helpers.corpus.failure_cases import all_failure_cases

out = Path(sys.argv[1])
shutil.rmtree(out, ignore_errors=True)
for case in all_failure_cases():
    target = out / f"failure__{case.name}"
    case.write(target)
    config = target / "sqlbuild_project.toml"
    if not config.exists():
        continue
    text = config.read_text()
    text = re.sub(r'adapter = "[^"]+"', 'adapter = "bigquery"', text, count=1)
    text = re.sub(r'\[connection\]\n(?:[^\[\n][^\n]*\n|\n)*', '[connection]\nproject = "example-project"\nlocation = "US"\n\n', text, count=1)
    if "[defaults]" in text:
        text = text.replace("[defaults]\n", '[defaults]\ndatabase = "example-project"\nschema = "analytics"\n', 1)
    else:
        text += '\n[defaults]\ndatabase = "example-project"\nschema = "analytics"\n'
    config.write_text(text)
print(len(list(out.iterdir())))
