#!/bin/bash
# Load nvm if available (handles IDEs that don't source .bashrc/.zshrc)
if [ -f "$HOME/.nvm/nvm.sh" ]; then
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && . "$NVM_DIR/nvm.sh"
fi
exec node_modules/.bin/vite "$@"
