#!/bin/bash
# Only in CI
if [ "$GITHUB_ACTIONS" ]; then
    git config --global user.email "github-actions[bot]@users.noreply.github.com"
    git config --global user.name "github-actions[bot]"
    echo "SHORT_DATE=$(date +%Y%m%d_%H%M)" >>$GITHUB_ENV
fi

# Default to 40 attempts if not set
attempts=${MAX_ATTEMPTS:-40}
BRANCH=${BRANCH:-tokenizer}

success=0

for i in $(seq 1 $attempts); do
    echo "=== Attempt $i/$attempts ===" | tee -a attempts.txt

    # Run tests and print output to console, capture to temp file
    cargo test -- --test accepts_model_from_config --test-threads=1 2>&1 | tee test_output.tmp
    test_exit_code=${PIPESTATUS[0]}

    # Trim output to only include failures section
    test_output=$(sed -n '/failures:/,/failures:/p' test_output.tmp | sed '1d; $d')
    rm test_output.tmp

    # Append trimmed test results to attempts.txt
    echo "$test_output" >>attempts.txt
    echo -e "\n\n" >>attempts.txt

    # Exit loop if tests passed
    if [ $test_exit_code -eq 0 ]; then
        success=1
        if [ "$GITHUB_ACTIONS" ]; then
            echo "ATTEMPTS=$i" >>$GITHUB_ENV
        fi
        break
    fi

    # Create temp file for askds input and clean it up
    askds_input=$(tail -c 250000 attempts.txt | sed 's/===/---/g')
    echo "$askds_input" >askds_input.tmp

    # Run askds and stream output to both console and variable
    echo "--- askds Output ---" | tee -a attempts.txt

    askds_output=$(
        askds \
            --hide-ui \
            --fix \
            --auto-apply \
            --serialize="yek --max-size=100KB | cat" \
            --test-file-pattern='tests/*.rs' \
            --source-file-pattern='src/**/*.rs' \
            --system-prompt=./prompts/fix-tests.txt \
            --run="cat askds_input.tmp" 2>&1 | tee /dev/stderr
    )
    askds_exit_code=$?

    if [ $askds_exit_code -ne 0 ]; then
        echo "askds failed with exit code $askds_exit_code" >>attempts.txt
        echo "askds failed. Guessing we ran out of context window. Trimming attempts.txt to last 30KB"
        tail -c 30000 attempts.txt >attempts.tmp
        mv attempts.tmp attempts.txt
        continue
    fi

    echo "$askds_output" >>attempts.txt
    echo "--- End askds Output ---" >>attempts.txt
    # Cleanup temp files
    rm askds_input.tmp

    # Commit changes if any
    if ! git diff --quiet; then
        git add .
        git commit -m "fix attempt $i (${BRANCH})"
        echo "Applied fixes for ${BRANCH} tests" | tee -a attempts.txt
    else
        echo "No changes in attempt $i" | tee -a attempts.txt
        continue
    fi
done

if [ $success -ne 1 ]; then
    if [ "$GITHUB_ACTIONS" ]; then
        echo "ATTEMPTS=$attempts" >>$GITHUB_ENV
        echo "::error::Failed after $attempts attempts"
        exit 1
    else
        echo "Failed after $attempts attempts"
        exit 1
    fi
fi
