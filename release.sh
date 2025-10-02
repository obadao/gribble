#!/bin/bash

# Gribble Release Script
# Usage: ./release.sh [version]
# Example: ./release.sh 1.1.0

set -e

if [ $# -eq 0 ]; then
    echo "Usage: $0 [version]"
    echo "Example: $0 1.1.0"
    exit 1
fi

VERSION=$1
TAG="v$VERSION"

# Validate version format (basic check)
if [[ ! $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Version must be in format X.Y.Z (e.g., 1.1.0)"
    exit 1
fi

# Check if we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo "Error: Must be on main branch to create a release. Current branch: $CURRENT_BRANCH"
    exit 1
fi

# Check if working directory is clean
if ! git diff-index --quiet HEAD --; then
    echo "Error: Working directory is not clean. Commit or stash your changes first."
    exit 1
fi

# Check if tag already exists
if git tag -l | grep -q "^$TAG$"; then
    echo "Error: Tag $TAG already exists"
    exit 1
fi

# Update version in Cargo.toml if it exists
if [ -f "Cargo.toml" ]; then
    echo "Updating version in Cargo.toml..."
    sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
    
    # Commit version update
    git add Cargo.toml
    git commit -m "Bump version to $VERSION"
fi

# Create and push tag
echo "Creating tag $TAG..."
git tag -a "$TAG" -m "Release $TAG"

echo "Pushing to origin..."
git push origin main
git push origin "$TAG"

echo ""
echo "✅ Release $TAG created and pushed!"
echo "🚀 GitHub Actions will now build and publish the release automatically."
echo "📦 Check the Actions tab to monitor the build progress."
