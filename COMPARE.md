

| backup software/config| delete          | dedup    |
|-----------------------|-----------------|----------|
| zfs snapshots + rsync | any             | optional |
| rsnapshot             | oldest only[^1] | no       |


[^1]: rsnapshot uses reverse diffs & does not provide a way to "expand" an
  intermediate reverse diff when deleting an intermediate snapshot
