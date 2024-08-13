# `pnpm-extra`

Just a few missing utilities in pnpm:

## `pnpm-extra tree`

Modelled on `cargo tree -i`, this is a better replacement for `pnpm why` that:
* shows an inverse dependency tree, which I find much more useful,
* always shows everything in the workspace,
* doesn't descend into dependencies that were already shown,
* also just gives a more compact output

## `pnpm-extra catalog add`

Like `pnpm add` but for adding catalog entries to `pnpm-workspace.yaml`.

Pretty early: doesn't preserve any comments or formatting, so it
runs `prettier` on the file afterwards.
