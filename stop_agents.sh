#!/usr/bin/env bash
# ⏹️  Stop all Fork-OS agents

echo "⏹️  Stopping all Fork-OS agents..."
echo ""

# Kill agents by PID files
for pid_file in agent_results/*.pid; do
    if [ -f "$pid_file" ]; then
        pid=$(cat "$pid_file")
        agent_name=$(basename "$pid_file" .pid)
        
        if kill -0 $pid 2>/dev/null; then
            echo "Stopping: $agent_name [PID: $pid]"
            kill $pid 2>/dev/null
            sleep 1
            
            # Force kill if still running
            if kill -0 $pid 2>/dev/null; then
                echo "  Force killing..."
                kill -9 $pid 2>/dev/null
            fi
        fi
        
        rm -f "$pid_file"
    fi
done

echo ""
echo "✅ All agents stopped"
