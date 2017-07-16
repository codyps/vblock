

| backup software/config| delete          | dedup        | storage efficiency |
|-----------------------|-----------------|--------------|--------------------|
| zfs snapshots + rsync | any             | optional[^1] | mid                |
| rsnapshot             | oldest only[^2] | no           |                    |

[^1]: zfs dedup in this context may not be quite as bad as using dedup in
  general. Can restrict to just the backup dataset.

[^2]: rsnapshot uses reverse diffs & does not provide a way to "expand" an
  intermediate reverse diff when deleting an intermediate snapshot
