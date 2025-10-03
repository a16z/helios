#!/bin/bash

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "Checking version consistency between Cargo.toml and helios-ts/package.json..."

# Function to extract version from Cargo.toml
get_workspace_version() {
    # First try direct version in [package] section
    local version=$(grep -E '^version = ' Cargo.toml 2>/dev/null | head -1 | sed 's/.*= "\(.*\)"/\1/' || true)
    
    # If no direct version, check workspace version
    if [ -z "$version" ]; then
        version=$(grep -A 5 '\[workspace.package\]' Cargo.toml | grep 'version = ' | sed 's/.*= "\(.*\)"/\1/' || true)
    fi
    
    echo "$version"
}

# Function to extract version from package.json
get_ts_version() {
    if command -v jq >/dev/null 2>&1; then
        jq -r '.version' helios-ts/package.json
    else
        # Fallback if jq is not available
        grep '"version"' helios-ts/package.json | sed 's/.*"version": "\(.*\)".*/\1/'
    fi
}

# Function to validate semantic version format
validate_version_format() {
    local version="$1"
    if ! echo "$version" | grep -E '^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9\-\.]*)?(\+[a-zA-Z0-9\-\.]*)?$' >/dev/null; then
        return 1
    fi
    return 0
}

# Main version check
main() {
    local cargo_version=$(get_workspace_version)
    local package_version=$(get_ts_version)
    
    echo -e "Cargo.toml workspace version: ${YELLOW}${cargo_version}${NC}"
    echo -e "helios-ts/package.json version: ${YELLOW}${package_version}${NC}"
    
    # Check if versions were found
    if [ -z "$cargo_version" ]; then
        echo -e "${RED}Could not find version in Cargo.toml${NC}"
        exit 1
    fi
    
    if [ -z "$package_version" ]; then
        echo -e "${RED}Could not find version in helios-ts/package.json${NC}"
        exit 1
    fi
    
    # Validate version formats
    if ! validate_version_format "$cargo_version"; then
        echo -e "${RED}Invalid version format in Cargo.toml: $cargo_version${NC}"
        echo "Version must follow semantic versioning (e.g., 1.2.3, 1.2.3-beta.1)"
        exit 1
    fi
    
    if ! validate_version_format "$package_version"; then
        echo -e "${RED}Invalid version format in package.json: $package_version${NC}"
        echo "Version must follow semantic versioning (e.g., 1.2.3, 1.2.3-beta.1)"
        exit 1
    fi
    
    # Compare versions
    if [ "$cargo_version" != "$package_version" ]; then
        echo -e "${RED}Version mismatch detected!${NC}"
        echo ""
        echo "The versions in the following files do not match:"
        echo "  • Cargo.toml [workspace.package]: $cargo_version"
        echo "  • helios-ts/package.json: $package_version"
        echo ""
        echo "To fix this, ensure both files have the same version specified."
        exit 1
    fi
    
    echo -e "${GREEN}Versions are consistent: $cargo_version${NC}"
    echo "Version check passed!"
}

# Run main function
main "$@"
