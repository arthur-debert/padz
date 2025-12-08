printf "Meeting Notes\n\nDiscussed the roadmap for Q4. Important topics included search UI.\n" > meeting.md
printf "Todo List\n\n1. Buy milk\n2. Fix search bug\n3. Review PR\n" > todo.md
printf "Rust Tips\n\nUse Vec::with_capacity for optimization.\nIterators are lazy.\n" > rust.md
printf "Short Title\n\nContent with search match here.\n" > short.md

padz import meeting.md todo.md rust.md short.md

echo "---------------------------------------------------"
echo "Checking all pads:"
padz list


echo "---------------------------------------------------"
echo "Results for 'search' (matches title and content):"
padz search "search"
echo "---------------------------------------------------"
echo "Results for 'optimization' (matches content with context):"
padz search "optimization"
echo "---------------------------------------------------"
echo "Results for 'Review' (matches content line):"
padz search "Review"
echo "---------------------------------------------------"
