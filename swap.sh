#!/usr/bin/env bash
#
# this allows you to easily save and restore your saturn db.
#

set -eou pipefail

if [ $# -ne 1 ]
then
  echo >&2 "usage: $0 [save|restore]"
  exit 1
fi

command=$1

case $command in
  clear)
    rm ~/.saturn.db
    rm ~/.saturn.conf
  ;;
  restore)
    mv ~/.saturn.db.save ~/.saturn.db
    mv ~/.saturn.conf.save ~/.saturn.conf
  ;;
  save)
    if [ -f ~/.saturn.db.save ] || [ -f ~/.saturn.conf.save ]
    then
      echo >&2 "Save file already exists"
      exit 1
    fi
    mv ~/.saturn.db ~/.saturn.db.save
    mv ~/.saturn.conf ~/.saturn.conf.save
  ;;
esac
