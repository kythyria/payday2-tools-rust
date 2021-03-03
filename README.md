This is a couple of tools for working with Payday 2. Currently included are the
`Idstring` hash function, searching the hashlist by hash, and a filesystem that
shows the contents of bundles as a volume.

As a result there are two dependencies:
+ An [up-to-date hashlist](https://github.com/Luffyyy/PAYDAY-2-Hashlist/).
  This will be read from `$CWD/hashlist` unless overridden with the `--hashlist`
  option
+ The [Dokany](https://dokan-dev.github.io/) userspace filesystem framework.

This is a CLI app, you are referred to the `--help` parameter for syntax.

# Credits
Contains a fork of https://github.com/RazrFalcon/xmlparser modified to be
spec-violatingly permissive.