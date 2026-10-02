# usage: state.sh <tag> <old-js> [spike env...]: put both windows in one state and screenshot them
S=$(dirname "$0"); tag=$1; js=$2; shift 2
pkill -x skidbladnir-gpu; pkill -x skidbladnir; sleep 1
(SKIDBLADNIR_NO_UPDATE_CHECK=1 WEBKIT_INSPECTOR_HTTP_SERVER=127.0.0.1:9333 ~/.local/bin/skidbladnir > $S/old.log 2>&1 &)
sleep 5; [ -n "$js" ] && { echo "$js" | $S/venv/bin/python $S/wk.py >/dev/null; sleep 1.2; }
bash $S/shoot.sh skidbladnir $S/old-$tag.png >/dev/null; pkill -x skidbladnir; sleep 1
(env SKIDBLADNIR_NO_UPDATE_CHECK=1 LD_LIBRARY_PATH=$S/libheif-gpl/lib "$@" $S/target/release/skidbladnir-gpuikit-spike > $S/spike.log 2>&1 &)
sleep 4; bash $S/shoot.sh skidbladnir-gpu $S/new-$tag.png >/dev/null
$S/venv/bin/python $S/diff.py $S/old-$tag.png $S/new-$tag.png $S/overlay-$tag.png
