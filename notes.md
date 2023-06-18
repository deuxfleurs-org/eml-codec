Test enron files:

```
cd resources/enron
./restore.sh
cd -
(set -euo pipefail; find resources/enron/maildir/ -type f | while read f; do echo $f; ./target/debug/imf_parse < $f > /dev/null; done)
```
