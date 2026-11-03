export type GitHubRun = {
	id: number;
	name: string;
	status: string;
	conclusion: string | null;
	updated_at: string;
	html_url: string;
};

export type GitHubActivity = {
	repository: string;
	workflow: string;
	label: string;
	run: GitHubRun;
};

export type GitHubNotice = {
	repository: string;
	workflow: string;
	title: string;
	body: string;
	url: string;
	priority: "critical" | "success";
};

export function currentGitHubActivity(
	repository: string,
	runs: GitHubRun[]
): GitHubActivity | null {
	const active = runs
		.filter((run) => run.status !== "completed")
		.sort((a, b) => Date.parse(b.updated_at) - Date.parse(a.updated_at))[0];
	if (!active) return null;
	const release = /release|publish|deploy/i.test(active.name);
	const label = release
		? "Publicando"
		: active.status === "queued"
			? `${active.name} · En cola`
			: `${active.name} · En curso`;
	return { repository, workflow: active.name, label, run: active };
}

export function githubRunTransitionNotice(
	repository: string,
	previous: Map<number, Pick<GitHubRun, "status" | "conclusion">>,
	current: GitHubRun[]
): GitHubNotice | null {
	const transitions: { notice: GitHubNotice; updatedAt: number }[] = [];
	for (const run of current) {
		const prior = previous.get(run.id);
		if (!prior || prior.status === "completed" || run.status !== "completed") continue;
		const failed = ["failure", "timed_out", "action_required"].includes(run.conclusion ?? "");
		if (failed) {
			transitions.push({
				notice: {
					repository,
					workflow: run.name,
					title: "GitHub necesita tu atención",
					body: `${run.name} · ${run.conclusion === "action_required" ? "Requiere acción" : "Falló"}`,
					url: run.html_url,
					priority: "critical"
				},
				updatedAt: Date.parse(run.updated_at)
			});
			continue;
		}
		if (run.conclusion === "success" && /release|publish|deploy/i.test(run.name)) {
			transitions.push({
				notice: {
					repository,
					workflow: run.name,
					title: "Publicación completada",
					body: `${repository} · ${run.name}`,
					url: run.html_url,
					priority: "success"
				},
				updatedAt: Date.parse(run.updated_at)
			});
		}
	}
	transitions.sort((a, b) => {
		const priority = (value: GitHubNotice["priority"]) => (value === "critical" ? 0 : 1);
		return priority(a.notice.priority) - priority(b.notice.priority) || b.updatedAt - a.updatedAt;
	});
	return transitions[0]?.notice ?? null;
}
