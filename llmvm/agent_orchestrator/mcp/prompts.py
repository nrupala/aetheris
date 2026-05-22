"""
Default MCP Prompt Templates for the Agent Orchestrator.

Templates:
- research_brief: Generate a research brief for a topic
- code_review: Review code for quality and correctness
- task_decomposition: Break down a complex task into subtasks
- agent_handoff: Hand off context between agents
- answer_synthesis: Synthesize findings into a final answer
"""

from .server import MCPPrompt, PromptLibrary


def register_default_prompts(library: PromptLibrary):
    """Register all default prompt templates."""
    
    library.add_prompt(MCPPrompt(
        name="research_brief",
        description="Generate a structured research brief for a topic",
        arguments=[
            {"name": "topic", "description": "The topic to research", "required": "true"},
            {"name": "scope", "description": "Scope of research (narrow/broad)", "required": "false"},
            {"name": "depth", "description": "Depth level (1-5)", "required": "false"},
        ],
        template="""You are a Research Agent. Generate a comprehensive research brief for the following topic.

Topic: {topic}
Scope: {scope}
Depth: {depth}

Structure your response as:
1. **Topic Overview** — Brief summary of the topic
2. **Key Questions** — 3-5 critical questions to answer
3. **Knowledge Gaps** — What information is missing
4. **Sources Used** — List relevant sources from context
5. **Recommendations** — Next steps for further research

Be thorough, cite sources, and identify gaps in information."""
    ))
    
    library.add_prompt(MCPPrompt(
        name="code_review",
        description="Review code for quality, correctness, and best practices",
        arguments=[
            {"name": "code", "description": "The code to review", "required": "true"},
            {"name": "language", "description": "Programming language", "required": "true"},
            {"name": "focus", "description": "Review focus (security/performance/readability)", "required": "false"},
        ],
        template="""You are a Review Agent. Review the following code for quality and correctness.

Language: {language}
Focus: {focus}

```{language}
{code}
```

Provide your review as:
1. **Overall Score** — Rate 1-10
2. **Strengths** — What's done well
3. **Issues** — Bugs, security concerns, or anti-patterns
4. **Suggestions** — Specific improvements with code examples
5. **Verdict** — Approve / Request Changes / Comment

Be constructive but thorough."""
    ))
    
    library.add_prompt(MCPPrompt(
        name="task_decomposition",
        description="Break down a complex task into manageable subtasks",
        arguments=[
            {"name": "task", "description": "The complex task to decompose", "required": "true"},
            {"name": "constraints", "description": "Any constraints or limitations", "required": "false"},
            {"name": "max_steps", "description": "Maximum number of subtasks", "required": "false"},
        ],
        template="""You are a Planning Agent. Break down the following complex task into actionable subtasks.

Task: {task}
Constraints: {constraints}
Max Steps: {max_steps}

Structure your plan as:
1. **Step 1**: [Description] — [Agent: researcher/coder/reviewer] — [Dependencies]
2. **Step 2**: [Description] — [Agent: researcher/coder/reviewer] — [Dependencies]
...

For each step specify:
- What needs to be done
- Which agent should execute it
- What it depends on
- Expected output format

Think strategically and identify all dependencies."""
    ))
    
    library.add_prompt(MCPPrompt(
        name="agent_handoff",
        description="Prepare context for handoff between agents",
        arguments=[
            {"name": "from_agent", "description": "Agent handing off", "required": "true"},
            {"name": "to_agent", "description": "Agent receiving", "required": "true"},
            {"name": "context", "description": "Context to pass", "required": "true"},
            {"name": "task", "description": "What the receiving agent should do", "required": "true"},
        ],
        template="""## Agent Handoff

**From**: {from_agent}
**To**: {to_agent}

### Context
{context}

### Task
{task}

### Instructions
Please review the context above and execute the specified task. 
Maintain all constraints and follow the established conventions."""
    ))
    
    library.add_prompt(MCPPrompt(
        name="answer_synthesis",
        description="Synthesize findings from multiple agents into a final answer",
        arguments=[
            {"name": "question", "description": "The original question", "required": "true"},
            {"name": "research_findings", "description": "Research agent findings", "required": "true"},
            {"name": "code_output", "description": "Code agent output (if applicable)", "required": "false"},
            {"name": "review_feedback", "description": "Reviewer feedback", "required": "false"},
        ],
        template="""## Answer Synthesis

**Question**: {question}

### Research Findings
{research_findings}

### Code Output
{code_output}

### Review Feedback
{review_feedback}

---

Synthesize all findings above into a comprehensive answer to the original question.

Structure:
1. **Direct Answer** — Clear, concise answer to the question
2. **Evidence** — Key findings that support the answer
3. **Implementation** — Code or implementation details (if applicable)
4. **Caveats** — Limitations or edge cases
5. **Sources** — Reference sources used

Be thorough but concise. Prioritize accuracy over completeness."""
    ))
