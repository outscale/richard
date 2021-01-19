#!/bin/bash
set -e

root=$(cd "$(dirname $0)" && pwd)

if [ ! -f "$root/config.env" ]; then
    echo "you need to copy config.env.ori to config.env and edit it, abort."
    exit 1
fi
source "$root/config.env"

if [ -z "$WEBEX_ACCESS_TOKEN" ]; then
    echo "need WEBEX_ACCESS_TOKEN environment variable to be set, abort."
    exit 1
fi

if [ -z "$ROOM_ID" ]; then
    echo "need ROOM_ID environment variable to be set, abort."
    exit 1
fi

# https://developer.webex.com/docs/api/basics
text=$@
data="
{
  \"roomId\": \"$ROOM_ID\",
  \"text\": \"$text\"
}
"

curl --request POST \
     --header "Authorization: Bearer $WEBEX_ACCESS_TOKEN" \
     --header "Content-Type: application/json" \
     --data "$data" \
     https://webexapis.com/v1/messages
