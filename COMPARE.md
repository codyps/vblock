

| backup software/config   | delete            | dedup in snapshots   | dedup between snaps   |snap atomicity | snap io cost | storage efficiency | bitrot resistance | data auth |
|--------------------------|-------------------|----------------------|-----------------------|----------------|--------------|--------------------|-------------------|-----------|
| zfs snapshots + zfs send | any               | optional, global[^1] | optional, via zfs     | fs via zfs     | none         | mid                | zfs scrub         |           |
| rsync + zfs snapshots    | any               | optional, global[^1] | optional, via zfs     | fs via zfs     | rsync scan   | mid                | zfs scrub         |           |
| rsnapshot                | oldest only[^2]   | no                   | reverse diffs         | none           | rsync scan   |                    |                   |           |
| time machine             | automatic only[^3]| no                   | file & dir hard links | ?              | ?            |                   |           |


[^1]: zfs deduplication is done on records, which are sized according to the content of the data they contain. In other words, the splitting is not content aware. zfs dedup is considered to be high-overhead, and constrains write performance. Unclear if this is a problem in a restricted backup usecase. Can restrict dedup to just the backup dataset (leaving other zfs use unaffected).

[^2]: rsnapshot uses reverse diffs & does not provide a way to "expand" an
  intermediate reverse diff when deleting an intermediate snapshot

[^3]: [apple documents](https://support.apple.com/en-us/HT201250) that time machine keeps hourly backups for 24hrs, daily backups for a month, and weekly backups for anything older than that. It then deletes the oldest backups first when the backup drive is full.
