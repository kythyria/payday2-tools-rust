This is a couple of tools for working with Payday 2. Currently included are the
`Idstring` hash function, searching the hashlist by hash, and a filesystem that
shows the contents of bundles as a volume.

As a result there are two dependencies:
+ An [up-to-date hashlist](https://github.com/Luffyyy/PAYDAY-2-Hashlist/).
  The search path is as follows: First try the `--hashlist` option, then `./hashlist`, then next to the binary.
+ Optionally, the [Dokany](https://dokan-dev.github.io/) userspace filesystem
  framework. Without this, `pd2tools-bundlefs.exe` will not run.

Two binaries are provided.

# Invocation
Because the tools are a work in progress, `--help` will print something different. The items not listed here may be defective, pointless, or merely accidentally omitted.

### `pd2tools hash ARG...`
Print the hash of each argument

### `pd2tools [-h HASHLIST] unhash [-d] ARG...`
Search the hashlist for each argument, trying both big and little endian.

If `-d` is specified, assume the hash is decimal, otherwise hex.

### `pd2tools [-h HASHLIST] scan ASSETS_DIR OUTPUT_FILE`
Analyse the asset bundles to locate possible hashlist entries, printing all candidates to a file. If any new levels have been added, it will take several iterations of adding the new entries to the hashlist and rerunning to pick up everything it's capable of detecting.

### `pd2tools convert --input-type FORMAT --output-type FORMAT INPUT_FILE [OUTPUT_FILE]`
Attempt to convert the `Binary` and `Custom`_xml scriptdata formats to `Generic`_xml or `Custom`_xml. This is a bit buggy, mostly useful for printing binary as generic_xml.

### `pd2tools-bundlefs [-h HASHLIST] ASSETS_DIR MOUNTPOINT`
**This requires Dokany.**

Read the hashlist, examine the bundle headers in `ASSETS_DIR`, and mount what's found as a **read-only** filesystem. Do note that WSL1 is unable to see it for some reason, the author doesn't know why.

The filesystem automatically renames `.texture` to `.dds` and `.movie` to `.bink`. Localisation string tables and most known scriptdata are automatically converted to textual form.

The filesystem will run until terminated by control-c or closing the terminal.

# Credits
Contains a fork of https://github.com/RazrFalcon/xmlparser modified to be
spec-violatingly permissive.