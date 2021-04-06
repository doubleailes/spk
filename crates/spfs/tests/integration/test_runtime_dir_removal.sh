#!/bin/bash
# test that a removed directory does not show up when running the env later

dirpath="/spfs/dir1/dir2/dir3";
to_remove="/spfs/dir1/dir2";
to_remain="/spfs/dir1";
base_tag="test/dir_removal_base";
top_tag="test/dir_removal_top";

spfs run - -- bash -c "mkdir -p $dirpath && spfs commit layer -t $base_tag"
spfs run -e $base_tag -- bash -c "rm -r $to_remove && spfs commit platform -t $top_tag"
spfs run test/dir_removal_top -- test ! -d $to_remove
spfs run test/dir_removal_top -- test -d $to_remain
