# Private comparison diagnostics

CLI comparison tests keep bounded evidence only when the existing delta/full comparator
reports a mismatch. Acceptance, output normalization, progress routing and cache policy are
unchanged. Successful comparisons create no evidence. Local evidence directories use mode
0700 and files use 0600. Streams and named environment credentials are redacted, but the
remaining project content must still be treated as private.

The CLI CI shard enables encrypted retention with `SQLBUILD_COMPARISON_ENCRYPTED_DIR` and
`SQLBUILD_COMPARISON_RECIPIENT`. The latter names the reviewed public recipient certificate;
no private key is available to the workflow. OpenSSL CMS uses AES-256-GCM and RSA-OAEP with
SHA-256 to encrypt each ZIP bundle. Only completed `failure.cms` files are uploaded, only
when the shard fails, with seven-day retention. Plaintext temporary evidence and partial
encryption output are never upload paths or printed by capture.

Each comparison series retains at most four failures of 4 MiB each, with individual streams
capped at 512 KiB. One shard reserves at most twelve ciphertext slots, each capped at
4 MiB + 64 KiB (48.75 MiB total). Slots are reserved atomically across test workers.
Truncation, original lengths/digests and capture errors are explicit in the evidence.
Encryption failure cannot replace the original comparison result and has no plaintext
upload fallback. A missing encrypted artifact is not evidence that a failed comparison
was captured successfully.

## Recover an encrypted failure

Download the encrypted GitHub artifact to a private machine. Obtain the matching private
key through the private secret store, never through GitHub, repository content or logs.
Decrypt to a temporary file, and rename it only after authentication succeeds:

```sh
umask 077
if openssl cms -decrypt -binary -inform DER -in failure.cms \
    -inkey /private/path/recipient-key.pem -out evidence.zip.partial; then
  mv evidence.zip.partial evidence.zip
else
  rm -f evidence.zip.partial
  exit 1
fi
```

Do not inspect or extract partial decryption output: OpenSSL may write unauthenticated
bytes before rejecting tampering. Wrong keys, truncated ciphertext and modified GCM tags
must fail. Extract only into a private directory; do not publish raw evidence.

The tracked certificate is public encryption material, not a trust root for authentication.
To rotate it, securely store and verify the replacement private key before replacing the
certificate. Retain prior private keys at least until every corresponding artifact has
expired. Neither key rotation nor diagnostics grants product access or changes CLI semantics.
