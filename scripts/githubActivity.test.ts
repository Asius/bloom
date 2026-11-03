import { describe, expect, test } from "bun:test";
import {
	currentGitHubActivity,
	githubRunTransitionNotice,
	type GitHubRun
} from "../src/githubActivity";

function run(overrides: Partial<GitHubRun> = {}): GitHubRun {
	return {
		id: 4,
		name: "CI",
		status: "in_progress",
		conclusion: null,
		updated_at: "2026-11-02T12:00:00Z",
		html_url: "https://github.com/Asius/bloom/actions/runs/4",
		...overrides
	};
}

describe("actividad y avisos de GitHub", () => {
	test("muestra cualquier workflow activo y prioriza el más reciente", () => {
		const activity = currentGitHubActivity("Asius/bloom", [
			run(),
			run({ id: 5, name: "Release", updated_at: "2026-11-02T12:02:00Z" })
		]);
		expect(activity?.label).toBe("Publicando");
		expect(currentGitHubActivity("Asius/bloom", [run({ status: "queued" })])?.label).toBe(
			"CI · En cola"
		);
		expect(currentGitHubActivity("Asius/bloom", [run({ status: "completed" })])).toBeNull();
	});

	test("solo notifica transiciones relevantes y pone fallos antes que publicaciones exitosas", () => {
		const prior = new Map([
			[4, { status: "in_progress", conclusion: null }],
			[5, { status: "in_progress", conclusion: null }]
		]);
		const notice = githubRunTransitionNotice("Asius/bloom", prior, [
			run({ id: 4, status: "completed", conclusion: "success" }),
			run({ id: 5, name: "Release", status: "completed", conclusion: "failure" })
		]);
		expect(notice?.priority).toBe("critical");
		expect(notice?.title).toBe("GitHub necesita tu atención");
		expect(
			githubRunTransitionNotice("Asius/bloom", new Map(), [
				run({ status: "completed", conclusion: "failure" })
			])
		).toBeNull();
		expect(
			githubRunTransitionNotice(
				"Asius/bloom",
				new Map([[4, { status: "in_progress", conclusion: null }]]),
				[run({ name: "CodeQL", status: "completed", conclusion: "success" })]
			)
		).toBeNull();
	});

	test("avisa cuando una publicación se completa", () => {
		const notice = githubRunTransitionNotice(
			"Asius/bloom",
			new Map([[4, { status: "in_progress", conclusion: null }]]),
			[run({ name: "Release", status: "completed", conclusion: "success" })]
		);
		expect(notice?.priority).toBe("success");
		expect(notice?.title).toBe("Publicación completada");
	});
});
