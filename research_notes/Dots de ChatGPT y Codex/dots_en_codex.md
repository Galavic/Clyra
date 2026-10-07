# Dots de ChatGPT y Codex

## Cómo un dot usa Codex ("Usa Codex y tus herramientas conectadas") — ¿es Codex el sandbox donde corre código?

### Takeaway
Un dot NO ejecuta el código de ingeniería directamente en su propia computadora en la nube: delega el trabajo de código a Codex (tareas locales o en un Codex cloud environment ya creado), mientras su computadora/buscador propios se usan para investigación, archivos y coordinación. Codex es, por tanto, el motor/sandbox de ejecución de código del dot.

### Cited Findings
- La página oficial de Dots afirma: "Your dot starts with context from your ChatGPT memory. It uses Codex and your connected tools to take on tasks" — [Source](https://chatgpt.com/features/dots/)
- El dot está "Powered by GPT-6 Astra, your dot lives in the cloud and has its own computer and browser" y "can research, analyze data, prepare documents, and build software, using relevant context from past conversations" — [Source](https://learn.chatgpt.com/docs/dots)
- "Your dot has its own computer and browser in the cloud for research, files, and running software. Cloud work can continue while your devices are off" — [Source](https://learn.chatgpt.com/docs/dots)
- "Your dot can use background agents, start new cloud threads, and create Work or Codex tasks on a connected computer. It can also continue existing local Codex tasks. For cloud coding work that needs your repository and setup, it can use a Codex cloud environment you've created" — [Source](https://learn.chatgpt.com/docs/dots)
- El dot "can use supported plugins installed and enabled for your account, with their connected accounts and existing permissions. For example, use Gmail..., Google Drive..., and GitHub to investigate code and prepare changes" — [Source](https://learn.chatgpt.com/docs/dots)
- "Connect your computer once through your dot's profile... This permission is separate from connecting your computer to Codex or enabling Work Sync" — [Source](https://learn.chatgpt.com/docs/dots)
- "Plugin permissions are shared across dots, ChatGPT, ChatGPT Work, and Codex. Your dot can use your existing connections within the permissions you've granted" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Codex Cloud "runs coding tasks in the cloud... Each new task gets its own isolated workspace from the published environment" y el environment es "the reusable setup that tasks use: repositories, dependencies, tools, and access settings" — [Source](https://learn.chatgpt.com/docs/environments/cloud-environments)
- "For cloud coding work, create the environment in Codex first, including its repository and setup configuration" — [Source](https://developers.openai.com/codex/dots/tasks-and-memory)
- "Conversations with your dot don't count toward your ChatGPT usage limits. When you ask your dot to start or manage tasks in Codex or ChatGPT Work, those tasks count toward your usage limits as usual" — [Source](https://openai.com/index/introducing-dots/)
- Dots "Through our ecosystem of plugins, they can readily connect to over 4,000 apps" — [Source](https://openai.com/index/introducing-dots/)
- Los dots funcionan con GPT-6 Astra, "un modelo orientado a investigación, programación y tareas complejas de varios pasos" y disponen de "computadora propia en la nube, navegador y acceso a aplicaciones conectadas" — [Source](https://www.infobae.com/tecno/2026/09/29/openai-lanzo-dots-agentes-de-ia-que-trabajan-las-24-horas-en-chatgpt/)

### Inferences
- La arquitectura es de dos capas: el dot (GPT-6 Astra, computadora cloud propia, memoria/notas, plugins) orquesta; Codex (harness + sandbox local o contenedor cloud) ejecuta el ciclo editar–probar–diff–PR.
- "Usa Codex y tus herramientas conectadas" significa en la práctica: el dot reutiliza las conexiones/plugins y permisos ya concedidos, no crea accesos nuevos por sí mismo.
- El permiso de "ordenador conectado" del dot y el permiso de "ordenador conectado a Codex" son independientes; hay que conceder ambos si se quiere que el dot opere tareas Codex locales.

### Gaps
- No se encontró documentación que describa el protocolo interno dot→Codex (¿llamada App Server, API interna, MCP?). Se describe solo a nivel de producto ("crear/gestionar tareas").
- No está claro si el dot puede crear un Codex cloud environment desde cero o solo usar uno ya publicado; la docs dice "you've created" / "you have already set up", lo que sugiere que el humano debe crearlo primero, pero no se afirma la imposibilidad de forma explícita.

## Qué superficies Codex existen y cómo aparecen los dots en cada una

### Takeaway
Codex existe en web/cloud, CLI, extensión IDE, app de escritorio y móvil, todo sobre el mismo harness (App Server); el dot vive en ChatGPT (desktop/web/móvil, Slack/Teams, voz) y aparece en Codex solo de forma indirecta: como creador/coordinador de tareas Codex visibles en Activity, hilos cloud y tareas locales.

### Cited Findings
- "Codex is OpenAI's coding agent that can read, edit, and run code... Use it side-by-side in your IDE or delegate larger tasks to the cloud" — [Source](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt)
- Superficies Codex documentadas: "ChatGPT desktop app (modo Codex), CLI de Codex, Extensión de Codex para IDE, Codex para la web" — [Source](https://help.openai.com/es-es/articles/11369540-using-codex-with-your-chatgpt-plan)
- "Use Codex across ChatGPT, your editor, and the terminal, all connected by your ChatGPT account" con secciones "Codex in ChatGPT / Codex cloud / Codex IDE extension" — [Source](https://openai.com/codex/)
- El harness común: "OpenAI's coding agent Codex exists across many different surfaces: the web app, the CLI, the IDE extension, and the new Codex macOS app. Under the hood, they're all powered by the same Codex harness" y el vínculo es "The Codex App Server, a client-friendly, bidirectional JSON-RPC" — [Source](https://openai.com/index/unlocking-the-codex-harness/)
- Hay una sola extensión oficial: "identifier `openai.chatgpt`, publisher OpenAI... covers VS Code, Cursor and Windsurf. JetBrains IDEs and Xcode reach Codex through their own built-in AI chat" — [Source](https://continuumcode.ai/guides/codex-cli-vscode/)
- La extensión IDE permite "Offload longer jobs to a cloud environment, then monitor progress and review results without leaving your IDE" y "Preview cloud changes, ask for follow-ups, and apply the resulting diffs locally" — [Source](https://marketplace.visualstudio.com/items?itemName=openai.chatgpt)
- Detalle IDE: "You can have Codex run from `main`... or run from your local changes" y "You can also view the cloud tasks in the Codex cloud interface" — [Source](https://developers.openai.com/codex/ide)
- CLI: "`codex cloud`: Move work to Codex cloud. Browse active and completed chats, submit work to a configured environment, and apply the result to your local repository from the terminal" — [Source](https://learn.chatgpt.com/docs/codex/cli)
- Cloud tasks: "Codex Cloud runs coding tasks on OpenAI-managed computers... you can start and continue tasks from desktop, web, or mobile" y "Each task starts from the prepared setup in its own isolated workspace" — [Source](https://help.openai.com/en/articles/20001545-using-codex-cloud)
- Visibilidad de tareas del dot: "Cloud threads appear as separate conversations in the desktop, web, and mobile apps, where you can review results... Local Codex tasks appear on the connected computer. You can keep talking to your dot while these tasks run" — [Source](https://learn.chatgpt.com/docs/dots)
- Inspección: "In the desktop app, open your dot's profile, select Activity, and open a task to inspect its progress, files, results, or requests for input... Your dot can continue coordinating the task when you step away" — [Source](https://learn.chatgpt.com/docs/dots)
- Tabla de 4 tipos de tarea del dot: nueva local Work/Codex, existente local Codex, nueva cloud coding, y tarea creada por el dot (revisar con follow-ups en su computadora/entorno original) — [Source](https://developers.openai.com/codex/dots/tasks-and-memory)
- "New cloud threads appear in your desktop, web, and mobile apps... Local Codex threads are available on the connected computer. For coding work that needs a particular repository and setup, your dot can use a Codex cloud environment you've created" — [Source](https://developers.openai.com/codex/dots/tasks-and-memory)
- Docs de Codex listan sección dedicada "dots: Meet dots / Getting started / Messaging / Tasks and memory / Computers and apps / Controls" dentro de developers.openai.com/codex — [Source](https://learn.chatgpt.com/docs/dots)
- Canales del dot: "You reach the same dot in ChatGPT, Slack, Teams, or a call... Messages stay in their respective channels" y en Slack "you can DM your dot or mention it in a channel you've added it to" — [Source](https://learn.chatgpt.com/docs/dots)

### Inferences
- No hay evidencia de que el dot tenga presencia nativa dentro del CLI o la extensión IDE como agente visible; su rastro en esas superficies son las tareas Codex (hilos cloud, diffs, PRs) que creó o coordina.
- La "sidebar de ChatGPT" mencionada en el encargo corresponde probablemente a dos piezas distintas: el panel Codex en el IDE / Codex en ChatGPT (web/app) y el perfil del dot (Activity/Scheduled) en la app de escritorio; la docs confirma Activity pero no usa el término "sidebar" para dots.

### Gaps
- No se encontró captura ni descripción de cómo se etiqueta una tarea Codex como "creada por el dot" en la UI web/CLI/IDE (¿atribución visible?).
- No se encontró documentación de dots dentro de chatgpt.com/es-419/codex (precios/overview/enterprise) más allá de la mención de uso compartido; la evidencia de pricing/planes viene de help y openai.com.
- Sin fuentes sobre OpenAI Academy o eventos específicos de "dots + Codex" (solo se listan eventos Codex genéricos).

## Cómo es el ciclo dot–Codex en la práctica (delegar, vigilar PRs/tests, revisar antes de actuar)

### Takeaway
El patrón documentado y ejemplar es: el humano da al dot una responsabilidad continua; el dot vigila fuentes (feedback/issues), delimita el fix más pequeño, lo implementa y prueba en Codex, abre PR (idealmente draft/fork, con descripción y grabación), y el humano revisa y fusiona; el dot aprende de los comentarios de revisión.

### Cited Findings
- Ejemplo oficial API-migration: "As an engineer, you need to move services across your company off an API before it shuts down. Your dot maps dependencies, prepares code and tests for engineers to review, and tracks remaining calls until they reach zero" (ilustración: "Todd tracks an API migration, with two passing pull requests and a breaking change to review") — [Source](https://chatgpt.com/features/dots/)
- Ejemplo feedback→código: "You're a developer building an app. Your dot watches customer feedback for requests, then scopes, builds, and tests improvements" (ilustración Jojo) — [Source](https://chatgpt.com/features/dots/)
- "OpenAI's own headline example: a dot that watches feedback and issues for repeat requests, scopes the smallest fix, builds and tests it in Codex, and opens a PR with a recording for you to review" — [Source](https://usesundog.com/workflow/feedback-to-scoped-pull-requests)
- Workflow concreto documentado por terceros a partir del anuncio: 1) "Create a Codex cloud environment for the repository first, so the dot has somewhere to build and test", 2) "Give the dot read access to the issue tracker and the support inbox or channel..., and a bot account with permission to open PRs on a fork" — [Source](https://usesundog.com/workflow/feedback-to-scoped-pull-requests)
- Brief reutilizable: "Each weekday morning, read new issues in [repo] and new messages in [feedback channel]. Group recurring requests and bugs. Pick the single smallest fix..., implement it in the Codex environment [name], run the test suite, and open a pull request from the bot account on a fork with description and recording... Do not merge, deploy, change CI or secrets, or touch anything marked production. Message me the PR link and a one-line summary" — [Source](https://usesundog.com/workflow/feedback-to-scoped-pull-requests)
- Ejemplo en docs: "Follow up on software feedback. Connect the feedback source and code project. Ask your dot to investigate reported problems, such as a missing item in a sidebar, and prepare a fix for code review before merging" — [Source](https://learn.chatgpt.com/docs/dots)
- Prensa: "para quienes desarrollan software, el dot puede analizar comentarios de clientes, detectar problemas reiterados, definir el alcance de correcciones y preparar solicitudes de incorporación de cambios con pruebas y videos" — [Source](https://www.infobae.com/tecno/2026/09/29/openai-lanzo-dots-agentes-de-ia-que-trabajan-las-24-horas-en-chatgpt/)
- Monitoreo: el dot "can decide when to pause and wake up to continue work" y "can divide work among background agents that run in parallel and report back to it. You can keep talking to your dot while they work" — [Source](https://developers.openai.com/codex/dots/tasks-and-memory)
- Revisión Codex disponible para el ciclo: "`@codex review`" en comentario de PR hace que "Codex posts a review on the pull request, just like a teammate would" y solo señala P0/P1; "Codex searches your repository for `AGENTS.md` files and follows the applicable code review rules" — [Source](https://learn.chatgpt.com/docs/third-party/github)
- Reglas de review: "Code review rules guide Codex; they don't replace tests, branch protections, or required approvals" — [Source](https://learn.chatgpt.com/docs/third-party/github)
- Patrón comunitario de automatización PR con Codex en 6 etapas (aislar en worktree, abrir draft PR con evidencia, pre-review `codex review`, review `@codex review`, reparar CI con `@codex` acotado, merge humano bajo branch protection + CODEOWNERS) — [Source](https://developertoolkit.ai/en/codex/ship/pr-automation/)
- Ejemplo de integración Slack→PR vía dot + runtime Codex (propuesta externa, no oficial): bug en Slack → dot lo convierte en job (submitjob/getjob/canceljob sobre MCP), worker Kubernetes lanza Codex en sandbox, reproduce bug, añade regression test, abre draft PR con credencial restringida, el dot devuelve el link y "Merge and deployment keep their own approval steps" — [Source](https://www.linkedin.com/posts/vitalylobachev_openai-agentengineering-codex-activity-7511376974477549569-8Cw9)

### Inferences
- El ciclo "revisar antes de actuar" tiene dos puntos de revisión: (1) el dot trae resultados/decisiones al humano ("brings back results... reaches out when a decision needs your judgment"), (2) Codex aporta revisión de código automatizada + CI/tests antes del merge humano.
- La recomendación práctica emergente (fork + bot account + no tocar producción/secretos/CI) no viene de OpenAI sino de guías comunitarias; es interpretación razonable del modelo de permisos pero no requisito oficial documentado.

### Gaps
- No se encontró documentación oficial paso a paso de "delegar tarea de ingeniería al dot" más allá de los ejemplos (offsite, sidebar-missing-item, API migration); falta el flujo exacto de prompts y estados.
- No hay métricas oficiales de éxito/calidad del ciclo dot→Codex→PR (tests que pasan, tasa de merge); solo ilustraciones de marketing.

## Límites, permisos y puntos de aprobación conocidos cuando el dot actúa vía Codex

### Takeaway
Los límites son en capas: permisos de app/plugin compartidos, Auto-review automático, Custom Rules configurables, salvaguardas nucleares no desactivables (passwords, etc.), restricciones de investigación proactiva (solo lectura), y requisitos operativos (ordenador online/app abierta, entorno cloud pre-creado, uso que consume cuota Codex).

### Cited Findings
- "Built-in safeguards, your existing ChatGPT app permissions, and automatic approval checks apply from the start. Before an action affects your accounts or shares information, review determines whether your dot can proceed, needs your approval, or must hand a step over to you" — [Source](https://learn.chatgpt.com/docs/dots)
- "Before your dot takes an action that could affect your accounts or share information, an automatic review checks it against your instructions, permissions, custom rules, and built-in safety requirements. The review determines whether the action can proceed, needs your approval, or includes a step you must do yourself. For example, you must change a password yourself" — [Source](https://developers.openai.com/codex/dots/controls)
- Custom Rules (4 comportamientos): "Take action without asking / Take action when you say so / Ask before taking action / Hand off to you", configurables en Settings > Personalization > Custom rules — [Source](https://developers.openai.com/codex/dots/controls)
- "Saved custom rules... They are instructions your dot tries to follow, and it can make mistakes. They don't grant access to an app or computer, override built-in safety requirements, or remove required confirmations such as approval to use a saved login" — [Source](https://developers.openai.com/codex/dots/controls)
- "Your dot can make mistakes, including when following your rules. Review its work and check important details before relying on the result" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Acciones sensibles: "The most sensitive actions like changing a password or transferring money require you to take over... Other actions like permanently deleting data or installing software may require approval each time... In some of these cases such as sending recurring messages, you can give your approval in advance" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- "Custom rules cannot turn off core safety requirements, such as when your dot asks you to take back over to change a password. Custom rules also cannot change the separate safety review system, Autoreview, or the restrictions on proactive research" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Investigación proactiva: "its tools cannot directly send messages to other people, change content through plugins, or control a browser or computer" y "Proactive research has additional restrictions" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- También: "That research is read-only; follow-up actions require the applicable permissions" y "The tools it uses for that research can't send messages, change app content, or control your browser or computer" — [Source](https://developers.openai.com/codex/dots/controls)
- Permisos de app: niveles "Always ask / Allow read actions / Allow low-risk actions / Allow all actions" y "Actions are evaluated against the available app capabilities, provider permissions, workspace policies, and safety protections. Some requests may be denied" — [Source](https://help.openai.com/en/articles/20001495-managing-app-permissions-in-chatgpt)
- Requisitos operativos Codex local: "Create a task on a computer connected to your dot. Keep that computer online with the ChatGPT app open" y "You can connect only one personal computer at a time" — [Source](https://developers.openai.com/codex/dots/tasks-and-memory)
- "Changing your dot's selected computer doesn't move an existing task to that computer" y "A new task receives instructions and context from your dot for that work. It has its own conversation; it doesn't automatically receive every conversation you've had with your dot" — [Source](https://developers.openai.com/codex/dots/tasks-and-memory)
- Parar trabajo tiene 3 efectos distintos: "Pause stops your dot's current main task. It doesn't stop every delegated task or cancel future scheduled runs. Open a delegated task in Activity to inspect and stop that task. Open Scheduled to disable or delete the recurring task" — [Source](https://developers.openai.com/codex/dots/controls)
- En propuesta de integración externa se subraya: "Only the owner can direct their dot through Slack. Pausing the dot does not automatically stop delegated work" — [Source](https://www.linkedin.com/posts/vitalylobachev_openai-agentengineering-codex-activity-7511376974477549569-8Cw9)
- Uso/cuota: "Tasks your dot starts or manages in Work or Codex count toward those products' usage limits as usual" y "Your plan includes an allowance for deeper work" — [Source](https://learn.chatgpt.com/docs/dots)
- Codex consume del "mismo fondo de uso de agentes y créditos" (Codex, Work, Excel, agentes del workspace); "El consumo de una tarea de Codex varía según el tamaño y la complejidad" — [Source](https://help.openai.com/es-es/articles/11369540-using-codex-with-your-chatgpt-plan)
- Permisos propios de Codex (sandbox + approvals): "The sandbox defines which files and network resources ChatGPT can access. Approvals determine when ChatGPT pauses... Changing who reviews a request doesn't expand the sandbox", modos "Ask for approval... Approve for me (Auto-review)... Full access", CLI "`/permissions`" — [Source](https://learn.chatgpt.com/docs/permission-modes.md)
- Seguridad Codex Cloud: "Runs in isolated OpenAI-managed containers... setup runs before the agent phase and can access the network..., then the agent phase runs offline by default unless you enable internet access"; "Secrets... are only available to setup scripts... removed before the agent phase" — [Source](https://developers.openai.com/codex/codex-manual.md)
- Admin Enterprise: "Dots are off by default and must be enabled by a workspace administrator", "Use dots (Beta)" + permisos de Slack/Teams, ordenador local, custom rules; "Local computer access is unavailable in workspaces with Codex or ChatGPT Work policies that target a specific operating system" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- Disponibilidad: "Pro 100, Pro 200, and Pro 500: for users over 18 outside EEA/UK/Switzerland. Business Premium: worldwide. Enterprise: worldwide [pero off by default]" — [Source](https://learn.chatgpt.com/docs/dots)

### Inferences
- El punto de aprobación más probable para código es doble: Auto-review del dot (¿puede abrir PR? ¿puede tocar X repo?) + permisos/approvals propios de Codex (sandbox, network, `/permissions`, branch protection, CODEOWNERS, CI requerido).
- La frase "Take action if pre-approved" para abrir PRs (guía Sundog) mapea al modo "Take action when you say so" oficial; es coherente pero la guía comunitaria simplifica el modelo real de 4 estados.
- El riesgo operativo principal es pausar el dot creyendo que se detuvo todo: las tareas Codex delegadas y los schedules siguen vivos y deben cancelarse por separado.

### Gaps
- No se encontró matriz oficial que diga qué acciones Codex vía dot requieren aprobación siempre (¿abrir PR? ¿pushear? ¿ejecutar tests?) frente a las que pueden pre-aprobarse; solo ejemplos (passwords, borrar datos, instalar software, compras, enviar mensajes).
- No se encontraron límites numéricos (nº de tareas Codex concurrentes por dot, timeout, tamaño de repo) ni detalle de cómo se muestra el consumo Codex iniciado por un dot en el panel de uso.
- Sin documentación en español (es-419) específica de dots+Codex; toda la evidencia primaria está en inglés (developers.openai.com, learn.chatgpt.com, help.openai.com en inglés, chatgpt.com/features/dots).
