#!/bin/bash
# Copyright (c) The Libra Core Contributors
# SPDX-License-Identifier: Apache-2.0

# adapted from --build-all-cti option from libra/docker/build-aws.sh, which is how
# land blocking test builds images before running cluster-test
REPOS=(libra_validator libra_cluster_test libra_init libra_safety_rules)
# the number of commits backwards we want to look
END=50

image_tag_exists() {
    if [ -z $1 ]; then
        echo "Missing image tag"
        exit 1
    fi
    image_tag=$1
    for (( i=0; i < ${#REPOS[@]}; i++ )); do
        repo=${REPOS[$i]}
        aws ecr describe-images --repository-name $repo --image-ids imageTag=$image_tag
        if [ $? -eq 0 ]; then
            echo "$repo:$image_tag found"
        else
            echo "$repo:$image_tag not found"
            return 1
        fi
    done
    echo "All images found for tag $image_tag"
    retval_image_tag=$image_tag
    return 0
}

for i in $(seq 0 $END); do
    test_tag=land_$(git rev-parse --short=8 origin/master~$i)
    image_tag_exists $test_tag && echo $retval_image_tag && exit 0
done

exit 1
