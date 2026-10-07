# Dots de ChatGPT — Notas de producto

## ¿Qué es exactamente un "dot" según OpenAI y cómo se posiciona frente a ChatGPT normal y frente a GPTs?

### Takeaway
Un dot es un agente always-on dentro de ChatGPT, con ordenador y navegador propios en la nube, motorizado por GPT-6 Astra, que asume responsabilidades continuas entre conversaciones; OpenAI lo presenta como "una forma totalmente nueva de trabajar con IA" y extensión del usuario, no como un chat que responde y se detiene.

### Cited Findings
- Los dots son "remarkably capable, always-on agents built to handle everything — powered by GPT-6 Astra" — [Source](https://chatgpt.com/features/dots/)
- Son "a whole new way to work with AI — one that gets to know what matters to you, is always working on your behalf, and takes important work off your plate" — [Source](https://openai.com/index/introducing-dots/)
- Son "frontier intelligence that have your back", tienen "their own cloud computer, learn from feedback over time, and can work towards your goals 24/7" — [Source](https://openai.com/index/introducing-dots/)
- "Think of your dot as an extension of you. It learns how you work, spots what needs attention, and uses its own computer to move your projects forward" — [Source](https://chatgpt.com/features/dots/)
- "Your dot starts with context from your ChatGPT memory. It uses Codex and your connected tools to take on tasks. You choose which apps it can access, what it should focus on, and how you like things done" — [Source](https://chatgpt.com/features/dots/)
- "Your dot is an always-on agent in ChatGPT that can take on ongoing responsibility and keep making progress between conversations. Powered by GPT-6 Astra, it has its own cloud computer, works across the apps you choose to connect, and remembers context" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Give your dot a goal and define what it can do on its own. It can work through complex problems, figure out what needs to happen next, and bring results back for your review. It turns to you for decisions that need your judgment" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Hand over a big project or something smaller to take off your plate. Your dot proactively figures out the next steps and keeps making progress between conversations" — [Source](https://chatgpt.com/features/dots/)
- "Your dot works between conversations. It tracks progress, works out what needs to happen next, and follows through as things change" — [Source](https://learn.chatgpt.com/docs/dots)
- "Your dot can decide when to pause and wake up to continue work; you don't need a fixed schedule for every follow-up. It can use background agents to work on several things in parallel while you keep talking to it" — [Source](https://learn.chatgpt.com/docs/dots)
- Se conectan a "over 4,000 apps" mediante el ecosistema de plugins — [Source](https://openai.com/index/introducing-dots/)
- "Dots run on GPT-6 Astra, our most aligned model, with safety and privacy protections on by default" — [Source](https://chatgpt.com/features/dots/)
- Capacidades destacadas: investigar, analizar datos, preparar documentos y construir software usando contexto de conversaciones pasadas y preferencias — [Source](https://learn.chatgpt.com/docs/dots)
- Ejemplos oficiales: finance lead que actualiza deck de inversores, sales lead que prepara evaluation plan, ingeniero que mapea dependencias de migración de API, developer que convierte feedback en fixes testeados, creador que genera clips y show notes — [Source](https://chatgpt.com/features/dots/)
- Ejemplo interno: "A bug appears in Slack, and dots immediately start investigating. A new design arrives, and dots turn it into a working app" y caso de tester cuyo dot detectó factura olvidada, la preparó y la envió tras aprobación — [Source](https://openai.com/index/introducing-dots/)
- "Your dot works on your behalf; specialist dots take on dedicated responsibilities within your organization" con identidad, credenciales y accesos propios — [Source](https://openai.com/index/introducing-dots/)
- Specialist dots en preview para access management, IT-provisioned hardware e integraciones profundas con systems of record; pilotos iniciales en procurement, invoice processing, email marketing, customer support y commercial contracting — [Source](https://openai.com/index/introducing-dots/)
- OpenAI trabaja con Microsoft para integrar specialist dots con gobernanza de Agent 365 — [Source](https://openai.com/index/introducing-dots/)
- Hoy se empieza con "your primary dot", con visión futura de "teams of dots working together on your behalf" — [Source](https://openai.com/index/introducing-dots/)
- Fechas: anuncio "Introducing dots" publicado el 29 de septiembre de 2026 tras presentación en DevDay anual — [Source](https://openai.com/index/introducing-dots/); página de producto indica "Dots are rolling out in ChatGPT" ya en web, móvil y escritorio — [Source](https://chatgpt.com/features/dots/)
- Frente a GPT personalizado, análisis secundario resume: GPT es "assistente configurado (instruções, arquivos e Actions) que responde quando você conversa" y dot es "agente sempre ativo, com computador e navegador próprios na nuvem, que continua trabalhando sem você" — [Source](https://horadecodar.com.br/dots-vs-gpts/); otro análisis advierte que a 30-sep-2026 OpenAI no había deprecado Custom GPTs ni publicado API de dots y que son patrones distintos (conversación configurada vs trabajo delegado persistente) — [Source](https://www.rewarx.com/blogs/the-death-of-custom-gpts-why-dots-make-static-chatbots-obsolete)

### Inferences
- El posicionamiento oficial evita comparar directamente con GPTs; enfatiza continuidad, proactividad y delegación frente al chat reactivo.
- La distinción dot personal vs specialist dot sugiere segmentación usuario individual vs responsabilidades organizativas con identidad propia.

### Gaps
- No se encontró en fuentes primarias consultadas una comparativa oficial dot vs GPTs ni declaración de retirada de GPTs; la información sobre migración GPT→plugin proviene solo de fuentes secundarias y documentación de navegación, sin confirmación en los artículos de dots revisados.
- No hay benchmarks públicos ni SLA de "24/7" más allá del marketing; sin cifras de tiempo de actividad o límites de cómputo.

## ¿Qué planes y mercados tienen acceso, cuánto cuesta y cómo lo habilita un admin?

### Takeaway
A 6-oct-2026 los dots están en rollout gradual solo para Pro, Business Premium y beta de Enterprise (incluye Edu y Healthcare) en mercados elegibles, con el primer dot incluido sin coste extra; Pro excluye EEE, Suiza y Reino Unido, Business Premium y Enterprise son worldwide, y Enterprise requiere activación por admin y está off por defecto.

### Cited Findings
- "Dots are rolling out today in ChatGPT to Pro and Business Premium users in eligible markets. Enterprise users (including Edu and Healthcare) can try the beta when their workspace admin enables it" — [Source](https://openai.com/index/introducing-dots/)
- "Dots are rolling out in ChatGPT on web, mobile, and desktop. Access is available now across Pro, Business Premium, and Enterprise plans in eligible markets. Enterprise users can try dots when enabled by their workspace admin" — [Source](https://chatgpt.com/features/dots/)
- "Dots are rolling out today in ChatGPT to Pro users in markets excluding the European Economic Area, Switzerland, and the UK. Dots are also available to Business Premium users across all supported ChatGPT regions" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Pro 100, Pro 200, and Pro 500: For users over 18 outside the European Economic Area, United Kingdom, and Switzerland" — [Source](https://learn.chatgpt.com/docs/dots)
- "Business Premium: Rolling out worldwide" — [Source](https://learn.chatgpt.com/docs/dots)
- "Enterprise: Rolling out worldwide. Dots are off by default and must be enabled by a workspace administrator" — [Source](https://learn.chatgpt.com/docs/dots)
- "Dots are rolling out gradually. Access may take several days to reach your account" y "You may not see dots immediately, even if your plan is eligible" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot); también en — [Source](https://learn.chatgpt.com/docs/dots)
- "Are dots available to users under 18? No. Dots are not yet available to users under 18" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- "Your first dot is included in your Pro or Business Premium plan at no extra cost. It's available 24/7 to talk, help you think, and stay on top of what matters to you" — [Source](https://openai.com/index/introducing-dots/)
- "Your plan also includes an allowance for deeper work, with extended limits for the first month after launch. In the future, you'll be able to add more dots, and scale the output of each dot by either increasing its speed or the total amount of work it can take on per month" — [Source](https://openai.com/index/introducing-dots/)
- "Conversations with your dot don't count toward your ChatGPT usage limits. When you ask your dot to start or manage tasks in Codex or ChatGPT Work, those tasks count toward your usage limits as usual" — [Source](https://openai.com/index/introducing-dots/)
- "Conversations with your dot don't count toward your ChatGPT usage limits. Tasks your dot starts or manages in Work or Codex count toward those products' usage limits as usual. Your plan includes an allowance for deeper work, with extended limits for the first month" — [Source](https://learn.chatgpt.com/docs/dots)
- Planes Plus, Free y Go no aparecen en las listas de elegibilidad de lanzamiento (solo Pro, Business Premium y Enterprise/Edu/Healthcare beta) — [Source](https://openai.com/index/introducing-dots/); guías comunitarias confirman ausencia de Free/Go/Plus/Business estándar en listas — [Source](https://dotsbase.com/eligibility/)
- Habilitación Enterprise: "Open Workspace settings > Permissions & roles. Go to Permissions > Workspace default > Workspace capabilities. Find Use dots (Beta), then review the dots permissions and computer capabilities below. Save your settings" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- "Dots access is off by default for Enterprise. The other dots permissions below take effect only when a member has access to dots" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- Permisos controlables: Use dots (Beta) permite usar dots, off por defecto; Add dots to Slack and Microsoft Teams permite unirse con identidad propia; Allow local computer access permite usar archivos locales y comandos, off por defecto; Use custom rules for dots permite añadir/editar reglas, off por defecto — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- Controles cloud bajo Cloud computer capabilities: Cloud browser use, Cloud network access, Cloud computer use, más Use password manager como control separado; aplican a dots y Work Cloud incluso si Work está desactivado — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- "A dot's cloud computer does not automatically inherit a member's local VPN, browser sign-ins, or device policies" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- "Enabling dots does not grant access to every app or website. Supported app connections that a member already uses in ChatGPT may be available to their dot" con plugin controls, app permissions y autorización de cada servicio — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- Para revocar: revisar Use dots (Beta) en default y todos los roles asignados directa o vía grupos y quitar cada concesión; un rol que lo otorgue mantiene acceso aunque otro lo tenga off — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- "Enterprise model controls and default model settings do not apply to dots" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- Para Slack se requieren ambos "Use dots (Beta)" y "Add dots to Slack and Microsoft Teams"; si el workspace Slack exige aprobación, un owner/app manager debe aprobar instalación; cada miembro completa setup de su dot — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)

### Inferences
- La exclusión geográfica solo afecta a Pro personal; la ruta para EEE/UK/Suiza es Business Premium o Enterprise según docs actuales.
- El modelo comercial inicial es "primer dot incluido + cuota de trabajo profundo" sin precio publicado para dots extra o más capacidad.

### Gaps
- No se encontraron precios numéricos oficiales de Pro 100/200/500 ni de Business Premium en las páginas de dots revisadas; cifras de prensa ($200 Pro, $100/$125 Business) no se incluyen por falta de fuente primaria.
- No hay cifras de la "allowance for deeper work" ni qué ocurre tras el primer mes extendido; OpenAI solo dice que compartirá términos futuros.
- No se aclara elegibilidad detallada para Edu/Healthcare con FedRAMP/EKM o residencia UAE más allá de menciones secundarias.

## ¿Cómo crea, configura y controla un usuario su dot (objetivos, foco, acceso a apps, permisos)?

### Takeaway
Se crea una sola vez en app de escritorio o web de escritorio con onboarding de nombre, avatar y conexiones; luego se le dan objetivos en lenguaje natural, se eligen apps/plugins y ordenador local, y se gobierna con Custom Rules, Auto-review, Activity View, pausa y reset.

### Cited Findings
- "To get started, create your first dot in the ChatGPT desktop app or your desktop browser, connect your apps, and let it introduce itself. After the initial setup, you'll be able to message it in the ChatGPT mobile app" — [Source](https://openai.com/index/introducing-dots/)
- "Create your dot in the ChatGPT desktop app or in ChatGPT on desktop web. Follow the onboarding prompts to get started. The desktop app is also available on Windows" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "You cannot currently create a dot on mobile, and dots are not supported on mobile web" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Use desktop to connect messaging channels, such as Slack or texting" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "You can give your dot a name during setup. Its default handle is @yourname-dot. When you name it, its handle becomes @yourname-agentname. To change its name or avatar later, select its name or avatar to open the profile, then select the pencil icon" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Give your dot a name and choose its shape, color, eyes, glasses, and accessories. You can change these anytime" y ejemplo "@tibo-alfred for a dot named Alfred" — [Source](https://learn.chatgpt.com/docs/dots)
- "You can choose from the available characters or select a pet for your dot. Your dot may also generate a pet for you" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Para dar tarea: "Describe what you want your dot to do and share the details it needs. For example, you can ask it to review your calendar, research a topic, or remind you about something later" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Para adjuntar: "To attach a file or photo, select + in the conversation" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Revisión: "Open your dot's profile on the desktop app to review activity under In progress, Scheduled, and Completed. You can also open your dot's computer from the profile to view and interact with it" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Activity View in the desktop app shows your dot's ongoing and delegated tasks. You can see task descriptions and their status" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- "Your dot has its own cloud computer. Access to your local computer is optional and starts turned off. To connect a computer, use the ChatGPT desktop app on that computer" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "You can connect only one personal computer at a time. The connection stays in place between tasks. Your computer must be online with the ChatGPT app open for your dot to use it" — [Source](https://learn.chatgpt.com/docs/dots)
- "When you connect a computer and confirm Allow access, your dot can access files and work on that computer from any of its messaging channels. Confirm Revoke access to stop" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Con acceso local puede "create Work or Codex tasks, use local skills, and use your local browser when its cloud browser is blocked" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Para código cloud: "Your dot can create cloud tasks in Codex cloud environments. Create the environment in Codex before asking your dot to use it" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- También puede "use background agents, start new cloud threads, and create Work or Codex tasks on a connected computer. It can also continue existing local Codex tasks" — [Source](https://learn.chatgpt.com/docs/dots)
- Apps: "Connected apps give your dot access to information it can use for your tasks. Your dot can also review that information proactively and form memories from it" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Gestión: "You can connect new plugins and manage existing ones in ChatGPT's Plugins tab. These connections are shared across dots, ChatGPT, ChatGPT Work, and Codex" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- "Plugin permissions are shared across dots, ChatGPT, ChatGPT Work, and Codex. Your dot can use your existing connections within the permissions you've granted" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Ejemplo: "use Gmail to find relevant email, Google Drive to work with documents, and GitHub to investigate code and prepare changes. Each plugin must be connected and permitted" — [Source](https://learn.chatgpt.com/docs/dots)
- "Connecting a messaging channel doesn't automatically grant access to your apps or computer" y tabla que distingue messaging channel vs connected app/plugin vs local computer — [Source](https://learn.chatgpt.com/docs/dots)
- "You can disconnect connected apps. Disconnecting an app does not delete information your dot has already obtained from it. To delete that information, you need to delete your dot" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Email: "You can connect your personal email account so your dot can use it for your tasks. At launch, you cannot give your dot its own standalone email address" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Memoria: "Your dot receives memories from ChatGPT and can create its own memories, including from connected apps" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Your dot can receive memories and recent conversation context from ChatGPT, and conversations with your dot can contribute to memory in ChatGPT" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- "Turning off Memory in ChatGPT stops that sharing. It does not delete information that your dot has already received" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- "Your dot's context does not retain credentials, images, or screenshots" y contenido cifrado en reposo y tránsito — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- "You currently cannot view, delete or directly modify individual dot memories, including specific details that enter the dot's context from plugins" y solo borrado total vía delete — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Proactive research: "When you aren't actively working with it, your dot looks for ways to help in the background. We call this 'proactive research'. It does this by using the apps you've already connected with tools that are restricted to be read-only, which means that they can't send messages, change app content, or control your browser or computer" — [Source](https://openai.com/index/introducing-dots/)
- Herramientas de research "cannot directly: Send messages to other people. Change content through plugins. Control a browser or computer" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Custom Rules: "Dots start with built-in rules for when to act independently and when to ask for approval. Custom Rules let you allow specific actions, require approval, or block them" — [Source](https://openai.com/index/introducing-dots/)
- Opciones en Customize → Custom rules: "Take action without asking, Take action if pre-approved, Ask before taking action, Hand off to you" donde pre-approved significa explícitamente solicitado en el prompt — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Custom rules cannot turn off core safety requirements, such as when your dot asks you to take back over to change a password. Custom rules also cannot change the separate safety review system, Autoreview, or the restrictions on proactive research" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Auto-review: "Dots use auto-review to check actions that could affect your accounts or share information against your instructions, Custom Rules, and safety requirements" y "Certain sensitive tasks, such as changing a password, always stay with you" — [Source](https://openai.com/index/introducing-dots/)
- Acciones más sensibles "like changing a password or transferring money require you to take over"; otras "like permanently deleting data or installing software may require approval each time"; mensajes recurrentes pueden pre-aprobarse con detalle de destinatario, contenido y condiciones — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Compras: "Your dot can also make purchases using a card you've saved on a merchant's website. These purchases require your approval" — [Source](https://help.openai.com/en/articles/20001529-dots-privacy-security-and-safety-faqs)
- Login web: flujo privado donde el dot pausa mientras el usuario introduce credenciales en formulario seguro sin exponerlas al modelo; opción Save to Passwords requiere confirmación para reutilizar; alternativa Take over para hacerlo uno mismo — [Source](https://learn.chatgpt.com/docs/dots)
- Pausa: "open the ••• menu in its profile and select Pause. To start it again, select Paused • Tap to resume" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Reset: "Reset deletes your dot, including its conversations, saved memories, and scheduled tasks" en 3 pasos (perfil → ••• → Reset → confirmar) y luego crear nuevo dot en desktop — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Tareas programadas: pedir reminder o check recurrente con zona horaria y duración/fecha fin; gestión en Recent activity o Scheduled para ver activas/pausadas/completadas y cambiar repetición/hora/notificaciones — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)

### Inferences
- El control combina tres capas: permisos de plugin heredados, reglas personalizadas y Auto-review + monitorización que no se pueden desactivar.
- Desconectar apps o pausar no equivale a borrar; solo el reset elimina contexto propio (pero no archivos/hilos ya creados en otros lugares).

### Gaps
- No hay documentación pública de sintaxis avanzada de Custom Rules ni lista exhaustiva de acciones soportadas/bloqueadas.
- No se detalla cuota de background agents paralelos ni retención temporal exacta de sesiones de navegador cloud.

## ¿En qué superficies trabajan los dots (ChatGPT web/móvil/escritorio, Slack, Teams, SMS)?

### Takeaway
El mismo dot es alcanzable en ChatGPT desktop, web y móvil (con llamada de voz) más Slack y Teams con contexto compartido; el SMS/texting es solo beta limitada próxima/en curso, y Enterprise no tiene mensajería telefónica al lanzamiento.

### Cited Findings
- "Working with your dot is as simple as having a conversation. You can message or call dots in ChatGPT on desktop, web, and mobile. They can also message you with progress, questions, or decisions that need you" — [Source](https://openai.com/index/introducing-dots/)
- "Message or call your dot in ChatGPT on web, mobile, and desktop. Share updates, ask questions, or change direction as you go. When something needs your attention, it gets in touch" — [Source](https://chatgpt.com/features/dots/)
- "Your dot is always within reach, whether it is through ChatGPT, Slack, or Teams. You can ask questions, explore ideas, and give feedback along the way; or you can simply hop on a voice call when you need to talk something out" — [Source](https://openai.com/index/introducing-dots/)
- "You can message your dot or call it to discuss work, change priorities, or make a decision. In ChatGPT, select the phone button in your dot's conversation to start a call. You can also type messages while you're talking. Work you've assigned can continue after the call ends" — [Source](https://learn.chatgpt.com/docs/dots)
- "Continue talking to your dot in ChatGPT on desktop or in the mobile app when the supporting update is available. Create your dot on desktop first; mobile web is not supported" — [Source](https://learn.chatgpt.com/docs/dots)
- "After creating your dot, you can talk to it in the ChatGPT mobile app when mobile access is available" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "On mobile, the computer opens in your control" al abrir el ordenador del dot desde el perfil — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "You reach the same dot in ChatGPT, Slack, Teams, or a call. Changing where you talk to it doesn't start a new dot or reset its memory. Messages stay in their respective channels, while your dot can use relevant context across them. Before sharing information from a private conversation with other people, your dot checks that you've allowed it" — [Source](https://learn.chatgpt.com/docs/dots)
- "You can also message your dot in Slack and Teams, with texting coming soon. Start a project in ChatGPT, share context with the working team in Slack, and let your dot follow through. Dots carry context across every channel" — [Source](https://openai.com/index/introducing-dots/)
- Página de producto muestra tabs ChatGPT / Slack / Teams con ejemplos: "Bring your dot into the channel to follow up" y "Bring your dot into Teams. Turn conversations into progress" — [Source](https://chatgpt.com/features/dots/)
- En Slack "you can DM your dot or mention it in a channel you've added it to. By default, it responds to you; you can instruct it to engage with others" — [Source](https://learn.chatgpt.com/docs/dots)
- "Tell it which channels and events to watch for. Adding it to a channel alone doesn't start monitoring" — [Source](https://learn.chatgpt.com/docs/dots)
- "You can also message your dot in Microsoft Teams" — [Source](https://learn.chatgpt.com/docs/dots)
- Permiso admin "Add dots to Slack and Microsoft Teams allows a dot to join supported Slack or Microsoft Teams workspaces and post with its own identity, where available" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- "Only a dot's owner can direct it. Other people's direct messages or mentions do not start work for that dot. When a dot posts in a channel, other channel members can see its messages" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- "Texting your dot is coming next" en la sección Right where you need it — [Source](https://chatgpt.com/features/dots/)
- "Where available, texting uses a third-party provider and is offered as a limited beta. Exercise caution when sharing sensitive information through text messages" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "The beta is limited to Pro users in the US and is not available in Business or Enterprise workspaces. Access is limited and may not be available to every Pro user" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "If texting is available for your account, you can use the desktop app to connect your phone and follow the setup prompts. After setup, you can message your dot from your phone" y para parar "Reply STOP" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Message and data rates may apply" para texting — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- "Phone messaging through iMessage, RCS, or WhatsApp is unavailable for Enterprise at launch" — [Source](https://help.openai.com/en/articles/20001554-manage-dots-in-chatgpt-workspaces)
- "Your dot cannot initiate calls to you at launch" y "You can connect your dot to Slack. Set up messaging channels from desktop" — [Source](https://help.openai.com/en/articles/20001530-getting-started-with-your-dot)
- Abrir ordenador cloud: "open its computer and select Take over. Select Return control when you're ready for your dot to continue" y cloud work continúa con dispositivos apagados — [Source](https://learn.chatgpt.com/docs/dots)
- "Cloud threads appear as separate conversations in the desktop, web, and mobile apps, where you can review results" — [Source](https://learn.chatgpt.com/docs/dots)

### Inferences
- La continuidad cross-canal es diferencial clave: mismo dot y memoria, pero hilos de mensajes separados por canal.
- Voz es entrante (usuario llama al dot); no hay llamadas proactivas del dot al lanzamiento.

### Gaps
- No hay fecha oficial para GA de texting/SMS fuera de beta US Pro ni lista de proveedores terceros.
- No se detalla disponibilidad de Slack/Teams por región o por tipo de workspace Slack (aprobación requerida) más allá de la nota de admin.
- Sin información sobre límites de notificaciones proactivas o cuotas de llamadas de voz.
