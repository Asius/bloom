import { getCurrentWindow } from "@tauri-apps/api/window";
import { GithubIcon } from "../icons";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
	Check,
	CheckCheck,
	Circle,
	ChevronDown,
	UserRound,
	Copy,
	ExternalLink,
	LoaderCircle,
	LogIn,
	LogOut,
	RefreshCw,
	Settings2,
	ShieldCheck,
	X
} from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import {
	getGithubDeviceFlowState,
	resetGithubDeviceFlow,
	startGithubDeviceFlow,
	subscribeGithubDeviceFlow
} from "../githubDeviceFlow";
import { PanelHeader } from "./PanelHeader";
import "./IslandGithub.css";
type Run = {
	id: number;
	name: string;
	path: string;
	status: string;
	conclusion: string | null;
	head_sha: string;
	updated_at: string;
	html_url: string;
};
type Job = {
	id: number;
	name: string;
	status: string;
	conclusion: string | null;
	steps?: { name: string; status: string; conclusion: string | null }[];
};
type Snapshot = { repository: string; runs: Run[] };
type RepoChoice = { id: number; fullName: string; private: boolean; selected: boolean };
type GithubAccount = { connected: boolean; login: string | null; selected: RepoChoice[] };
type ReleaseVerification = {
	version: string;
	verified: boolean;
	bytes: number;
	detail: string;
};
function state(status: string, conclusion: string | null): string {
	if (status !== "completed") return status === "queued" ? "En cola" : "En curso";
	return (
		(
			{
				success: "Correcto",
				failure: "Falló",
				cancelled: "Cancelado",
				skipped: "Omitido",
				timed_out: "Tiempo agotado",
				action_required: "Requiere acción"
			} as Record<string, string>
		)[conclusion || ""] || "Finalizado"
	);
}
export function IslandGithub({
	previous,
	next,
	onHeightChange,
	scrollable = false
}: {
	previous: () => void;
	next: () => void;
	onHeightChange?: (height: number) => void;
	scrollable?: boolean;
}) {
	const [editing, setEditing] = useState(false);
	const [source, setSource] = useState<"public" | "account">("public");
	const [privacy, setPrivacy] = useState(false);
	const [pickerOpen, setPickerOpen] = useState(false);
	const [search, setSearch] = useState("");
	const [draftRepo, setDraftRepo] = useState("");
	const [recentRepos, setRecentRepos] = useState<string[]>(["Asius/bloom"]);
	const panelRef = useRef<HTMLElement>(null);
	const contentRef = useRef<HTMLDivElement>(null);
	const [showOther, setShowOther] = useState(false);
	const [repository, setRepository] = useState("Asius/bloom");
	const [input, setInput] = useState("Asius/bloom");
	const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
	const [error, setError] = useState("");
	const [updated, setUpdated] = useState<number | null>(null);
	const [selected, setSelected] = useState<number | null>(null);
	const [jobs, setJobs] = useState<Job[]>([]);
	const [jobError, setJobError] = useState("");
	const [jobsLoaded, setJobsLoaded] = useState(false);
	const [account, setAccount] = useState<GithubAccount>({
		connected: false,
		login: null,
		selected: []
	});
	const [repoChoices, setRepoChoices] = useState<RepoChoice[]>([]);
	const [authBusy, setAuthBusy] = useState(false);
	const [repoError, setRepoError] = useState("");
	const [authMessage, setAuthMessage] = useState("");
	const [deviceFlow, setDeviceFlow] = useState(getGithubDeviceFlowState);
	const [codeCopied, setCodeCopied] = useState(false);
	const [verification, setVerification] = useState<ReleaseVerification | null>(null);
	const [verificationError, setVerificationError] = useState("");
	const [verifying, setVerifying] = useState(false);
	const repositoryGeneration = useRef(0);
	const refreshAfterAuthorization = useRef(false);
	const handledAuthorization = useRef(deviceFlow.completedAt);
	const generation = useRef(0);
	const jobGeneration = useRef(0);
	const reloadAccount = useCallback(async () => {
		const current = await invoke<GithubAccount>("github_auth_status");
		setAccount(current);
		return current;
	}, []);
	const loadRepositories = useCallback(async () => {
		const request = ++repositoryGeneration.current;
		setAuthBusy(true);
		setRepoError("");
		try {
			const result = await invoke<{ repositories: RepoChoice[]; warning: string | null }>(
				"github_repositories"
			);
			if (request === repositoryGeneration.current) {
				setRepoChoices(result.repositories);
				setRepoError(result.warning ?? "");
			}
		} catch (e) {
			if (request === repositoryGeneration.current) setRepoError(String(e));
		} finally {
			if (request === repositoryGeneration.current) setAuthBusy(false);
		}
	}, []);
	const manageRepositories = async () => {
		setAuthBusy(true);
		setAuthMessage("");
		try {
			refreshAfterAuthorization.current = true;
			await invoke("github_manage_repositories");
			setAuthMessage("Elige los repositorios en GitHub y vuelve a Bloom para actualizar la lista.");
		} catch (reason) {
			refreshAfterAuthorization.current = false;
			setAuthMessage(String(reason));
		} finally {
			setAuthBusy(false);
		}
	};
	useEffect(() => {
		let cancelled = false;
		void Promise.all([reloadAccount(), invoke<Record<string, string>>("load_settings")])
			.then(([current, settings]) => {
				if (cancelled) return;
				const value = settings["bloom-github-repository"];
				if (value !== undefined) {
					setInput(value);
					setRepository(value);
				}
				setSource(
					settings["bloom-github-source"] === "public"
						? "public"
						: current.connected
							? "account"
							: "public"
				);
			})
			.catch(() => {});
		return () => {
			cancelled = true;
		};
	}, [reloadAccount]);
	useEffect(() => {
		const current = repoChoices.find((repo) => repo.fullName === repository);
		if (current) setPrivacy(current.private);
	}, [repository, repoChoices]);
	const switchSource = (value: "public" | "account") => {
		setSource(value);
		setPickerOpen(false);
		if (
			value === "public" &&
			repoChoices.some((repo) => repo.fullName === repository && repo.private)
		) {
			setInput("");
			setRepository("");
			void invoke("save_setting", { key: "bloom-github-repository", value: "" })
				.then(() => emit("github-repository-changed", ""))
				.catch(() => setError("No se pudo guardar el repositorio público."));
		}
		void invoke("save_setting", { key: "bloom-github-source", value }).catch(() =>
			setError("No se pudo guardar la vista de repositorios.")
		);
	};

	useEffect(() => subscribeGithubDeviceFlow(setDeviceFlow), []);
	useEffect(() => {
		if (!deviceFlow.completedAt || deviceFlow.completedAt <= handledAuthorization.current) return;
		handledAuthorization.current = deviceFlow.completedAt;
		setAuthMessage(`Sesión iniciada como ${deviceFlow.login}.`);
		void reloadAccount();
	}, [deviceFlow.completedAt, deviceFlow.login, reloadAccount]);
	useEffect(
		() => () => {
			repositoryGeneration.current++;
		},
		[]
	);
	useEffect(() => {
		if (account.connected) void loadRepositories();
	}, [account.connected, loadRepositories]);
	useLayoutEffect(() => {
		const panel = panelRef.current;
		const content = contentRef.current;
		if (!panel || !content || !onHeightChange) return;
		const measure = () => {
			const header = panel.querySelector("header");
			onHeightChange(content.offsetHeight + (header?.offsetHeight ?? 48) + 30);
		};
		const observer = new ResizeObserver(measure);
		observer.observe(content);
		if (panel.firstElementChild) observer.observe(panel.firstElementChild);
		measure();
		return () => observer.disconnect();
	}, [onHeightChange]);
	const beginLogin = async () => {
		setAuthBusy(true);
		setAuthMessage("");
		await startGithubDeviceFlow();
		setAuthBusy(false);
	};
	const copyCode = async () => {
		if (!deviceFlow.userCode) return;
		try {
			await writeText(deviceFlow.userCode);
			setCodeCopied(true);
			window.setTimeout(() => setCodeCopied(false), 1800);
		} catch {
			setAuthMessage("No se pudo copiar el código. Puedes seleccionarlo y copiarlo manualmente.");
		}
	};
	const useAccountRepository = async (repository: RepoChoice) => {
		const selection = repoChoices.filter((item) => item.selected || item.id === repository.id);
		setAuthBusy(true);
		try {
			await invoke("github_select_repositories", { repositories: selection });
			await invoke("save_setting", {
				key: "bloom-github-repository",
				value: repository.fullName
			});
			setInput(repository.fullName);
			setRepository(repository.fullName);
			setDraftRepo("");
			setRepoChoices((repos) =>
				repos.map((repo) => ({ ...repo, selected: selection.some((item) => item.id === repo.id) }))
			);
			await emit("github-repository-changed", repository.fullName);
			setPickerOpen(false);
			setError("");
			await reloadAccount();
		} catch (reason) {
			setAuthMessage(String(reason));
		} finally {
			setAuthBusy(false);
		}
	};
	const disconnect = async () => {
		repositoryGeneration.current++;
		setAuthBusy(true);
		try {
			await invoke("github_disconnect");
			resetGithubDeviceFlow();
			setAccount({ connected: false, login: null, selected: [] });
			setRepoChoices([]);
			setRepoError("");
			setSource("public");
			setAuthMessage("La cuenta y sus tokens protegidos se quitaron de este equipo.");
		} catch (e) {
			setAuthMessage(String(e));
		} finally {
			setAuthBusy(false);
		}
	};
	const verifyRelease = async () => {
		setVerifying(true);
		setVerification(null);
		setVerificationError("");
		try {
			setVerification(await invoke<ReleaseVerification>("github_release_verification"));
		} catch (e) {
			setVerificationError(String(e));
		} finally {
			setVerifying(false);
		}
	};
	const refresh = useCallback(async () => {
		const request = ++generation.current;
		if (!repository) {
			setSnapshot(null);
			setError("");
			return;
		}
		try {
			const result = await invoke<Snapshot>("github_monitor_snapshot", { repository });
			if (request !== generation.current) return;
			setSnapshot(result);
			setUpdated(Date.now());
			setError("");
		} catch (e) {
			if (request === generation.current) setError(String(e));
		}
	}, [repository]);
	useEffect(() => {
		setSnapshot(null);
		setSelected(null);
		setJobs([]);
		setUpdated(null);
		void refresh();
		const timer = setInterval(() => void refresh(), 120000);
		return () => {
			clearInterval(timer);
			generation.current++;
			jobGeneration.current++;
		};
	}, [refresh]);
	const selectRepository = async () => {
		const value = input.trim();
		if (
			repoChoices.some(
				(repo) => repo.fullName.toLowerCase() === value.toLowerCase() && repo.private
			)
		) {
			setError("Elige los repositorios privados desde Mi cuenta.");
			return;
		}
		if (value && !/^[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+$/.test(value)) {
			setError("Usa el formato propietario/repositorio.");
			return;
		}
		try {
			await invoke("save_setting", { key: "bloom-github-repository", value });
			setRepository(value);
			setRecentRepos((repos) => [...new Set([value, ...repos])].filter(Boolean).slice(0, 8));
			await emit("github-repository-changed", value);
			setPickerOpen(false);
			setError("");
		} catch {
			setError("No se pudo guardar el repositorio.");
		}
	};
	useEffect(() => {
		const request = ++jobGeneration.current;
		if (selected === null) return;
		setJobs([]);
		setJobsLoaded(false);
		setJobError("");
		void invoke<{ jobs: Job[] }>("github_monitor_jobs", { repository, runId: selected })
			.then((result) => {
				if (request === jobGeneration.current) {
					setJobs(result.jobs);
					setJobsLoaded(true);
				}
			})
			.catch((e) => {
				if (request === jobGeneration.current) setJobError(String(e));
			});
		return () => {
			jobGeneration.current++;
		};
	}, [repository, selected, updated]);
	const names = ["CI", "CodeQL", "Release"];
	for (const run of snapshot?.runs ?? []) {
		if (
			run.name &&
			!names.some((name) => name.toLowerCase() === run.name.toLowerCase()) &&
			names.length < 8
		)
			names.push(run.name);
	}
	const rows = names.map((name) => ({
		name,
		run: snapshot?.runs.find((r) => r.name.toLowerCase() === name.toLowerCase())
	}));
	const accountView = source === "account";
	const choices = repoChoices.filter(
		(repo) => repo.private === privacy && repo.fullName.toLowerCase().includes(search.toLowerCase())
	);
	const selectedChoice = repoChoices.find((repo) => repo.fullName === (draftRepo || repository));
	const openRepository = () =>
		void invoke("github_monitor_open", {
			repository: accountView ? (selectedChoice?.fullName ?? repository) : input.trim()
		}).catch((reason) => setError(String(reason)));
	return (
		<section
			ref={panelRef}
			className="github-panel"
			aria-label="Seguimiento de GitHub"
			onMouseEnter={() => {
				if (refreshAfterAuthorization.current && account.connected) {
					refreshAfterAuthorization.current = false;
					void loadRepositories();
				}
			}}
			onClick={(e) => e.stopPropagation()}
		>
			<PanelHeader
				title={
					accountView && account.connected
						? `GitHub · ${account.login}`
						: repository
							? `GitHub · ${repository.toLowerCase() === "asius/bloom" ? "Bloom" : repository.split("/")[1]}`
							: "GitHub"
				}
				icon={<GithubIcon size={25} />}
				subtitle={
					accountView && account.connected ? (
						<span className="github-connected">
							<i aria-hidden="true" />
							Cuenta conectada
						</span>
					) : (
						repository
					)
				}
				previous={previous}
				next={next}
				actions={
					<>
						<button
							title="Opciones de GitHub"
							aria-label="Opciones de GitHub"
							aria-expanded={editing}
							onClick={() => setEditing((v) => !v)}
						>
							<Settings2 size={16} />
						</button>
						<button
							title="Actualizar"
							aria-label="Actualizar"
							onClick={() => {
								void refresh();
								if (account.connected) void loadRepositories();
							}}
						>
							<RefreshCw size={16} />
						</button>
					</>
				}
			/>
			<div className="github-content" style={{ overflowY: scrollable ? "auto" : "hidden" }}>
				<div ref={contentRef} className="github-content-inner">
					<div className="github-source-tabs" aria-label="Origen del repositorio">
						<button
							disabled={deviceFlow.pending}
							aria-pressed={!accountView}
							onClick={() => {
								switchSource("public");
							}}
						>
							Repositorio público
						</button>
						<button
							disabled={deviceFlow.pending}
							aria-pressed={accountView}
							onClick={() => {
								switchSource("account");
							}}
						>
							Mi cuenta
						</button>
					</div>
					{accountView && (
						<div className="github-auth-card">
							<div className="github-auth-heading">
								<span className="github-user-icon">
									<UserRound size={23} />
								</span>
								<div className="github-auth-identity">
									<strong>
										{account.connected ? `Conectado como ${account.login}` : "Cuenta GitHub"}
									</strong>
									<span>
										{account.connected
											? "Sesión confirmada"
											: "Conecta tu cuenta para elegir sus repositorios"}
									</span>
								</div>
								{account.connected ? (
									<button disabled={authBusy} onClick={() => void disconnect()}>
										<LogOut size={15} /> Desconectar
									</button>
								) : (
									<button
										disabled={authBusy || deviceFlow.pending}
										onClick={() => void beginLogin()}
									>
										<LogIn size={15} />
										{deviceFlow.pending ? "Esperando a GitHub" : "Iniciar sesión"}
									</button>
								)}
							</div>
							{deviceFlow.userCode && (
								<div className="github-device-code-row">
									<p className="github-device-code">
										Código de autorización <strong>{deviceFlow.userCode}</strong>
									</p>
									<button className="github-copy-code" onClick={() => void copyCode()}>
										{codeCopied ? <CheckCheck size={15} /> : <Copy size={15} />}
										{codeCopied ? "Copiado" : "Copiar código"}
									</button>
								</div>
							)}
							{!account.connected && deviceFlow.message && (
								<p className="github-auth-message" role="status">
									{deviceFlow.message}
								</p>
							)}
							{deviceFlow.error && (
								<p className="github-error" role="status">
									{deviceFlow.error}
								</p>
							)}
						</div>
					)}
					{repoError && (
						<p className="github-error" role="alert">
							{repoError}
						</p>
					)}
					{authMessage && (
						<p className="github-auth-message" role="status">
							{authMessage}
						</p>
					)}
					{(!accountView || account.connected) && (
						<div className="github-repository">
							<h3>{accountView ? "Repositorios de tu cuenta" : "Repositorio público"}</h3>
							{accountView && (
								<div className="github-privacy-tabs" aria-label="Visibilidad de repositorios">
									<button
										aria-pressed={!privacy}
										onClick={() => {
											setPrivacy(false);
											setDraftRepo("");
										}}
									>
										Públicos
									</button>
									<button
										aria-pressed={privacy}
										onClick={() => {
											setPrivacy(true);
											setDraftRepo("");
										}}
									>
										Privados
									</button>
								</div>
							)}
							<div className="github-selector-row">
								<div className="github-selector">
									{accountView ? (
										<button
											className="github-selector-value"
											aria-label="Elegir repositorio de tu cuenta"
											aria-expanded={pickerOpen}
											onClick={() => setPickerOpen((v) => !v)}
										>
											{selectedChoice?.private === privacy
												? selectedChoice.fullName
												: "Selecciona un repositorio"}
										</button>
									) : (
										<input
											aria-label="Repositorio público"
											value={input}
											onChange={(e) => setInput(e.target.value)}
											onKeyDown={(e) => {
												if (e.key === "Enter") void selectRepository();
											}}
										/>
									)}
									<button
										className="github-selector-arrow"
										aria-label="Mostrar repositorios"
										aria-expanded={pickerOpen}
										onClick={() => setPickerOpen((v) => !v)}
									>
										<ChevronDown size={18} />
									</button>
								</div>
								<button
									className="github-icon-button"
									title="Abrir repositorio en GitHub"
									aria-label="Abrir repositorio en GitHub"
									onClick={openRepository}
								>
									<ExternalLink size={18} />
								</button>
								{accountView && (
									<button
										className="github-icon-button"
										title="Usar repositorio elegido"
										aria-label="Usar repositorio elegido"
										disabled={authBusy || !selectedChoice || selectedChoice.private !== privacy}
										onClick={() => selectedChoice && void useAccountRepository(selectedChoice)}
									>
										<Check size={20} />
									</button>
								)}
							</div>
							{pickerOpen && (
								<div className="github-picker">
									{accountView ? (
										<>
											<input
												autoFocus
												aria-label="Buscar repositorios de tu cuenta"
												onPointerDown={() => void getCurrentWindow().setFocus()}
												placeholder="Buscar repositorio…"
												value={search}
												onChange={(e) => setSearch(e.target.value)}
											/>
											{choices.map((repo) => (
												<button
													key={repo.id}
													onClick={() => {
														setDraftRepo(repo.fullName);
														setPickerOpen(false);
													}}
												>
													<span>{repo.fullName}</span>
													{(draftRepo || repository) === repo.fullName && <Check size={16} />}
												</button>
											))}
											{!choices.length && (
												<p>
													{authBusy
														? "Cargando repositorios…"
														: privacy
															? "No hay repositorios privados autorizados. Usa «Autorizar repositorios de Bloom» para elegirlos en GitHub."
															: "No hay repositorios que coincidan con la búsqueda."}
												</p>
											)}
										</>
									) : (
										recentRepos.map((repo) => (
											<button
												key={repo}
												onClick={() => {
													setInput(repo);
													setPickerOpen(false);
												}}
											>
												<span>{repo}</span>
												{repository === repo && <Check size={16} />}
											</button>
										))
									)}
								</div>
							)}
							{!accountView && input.trim() !== repository && (
								<button className="github-apply-public" onClick={() => void selectRepository()}>
									<Check size={15} /> {input.trim() ? "Usar repositorio" : "Quitar repositorio"}
								</button>
							)}
							{accountView &&
								(editing || (privacy && !repoChoices.some((repo) => repo.private))) && (
									<button
										className="github-install-app"
										disabled={authBusy || deviceFlow.pending}
										onClick={() => void manageRepositories()}
									>
										Autorizar repositorios de Bloom <ExternalLink size={14} />
									</button>
								)}
						</div>
					)}

					{error && (
						<p className="github-error" role="alert">
							{error}
							{updated ? " · Mostrando datos anteriores" : ""}
						</p>
					)}
					{!snapshot && !error && <p>Consultando GitHub…</p>}
					{snapshot &&
						rows
							.filter(({ name }) => showOther || ["CI", "CodeQL", "Release"].includes(name))
							.map(({ name, run }) => (
								<div className="github-run" key={name}>
									<button
										disabled={!run}
										onClick={() =>
											run && setSelected((current) => (current === run.id ? null : run.id))
										}
										aria-expanded={!!run && selected === run.id}
									>
										<span
											className={`github-state ${run?.conclusion === "success" ? "success" : run?.conclusion === "failure" ? "failure" : ""}`}
										>
											{run?.status && run.status !== "completed" ? (
												<LoaderCircle className="github-spin" size={21} />
											) : run?.conclusion === "success" ? (
												<Check size={15} />
											) : run?.conclusion === "failure" ? (
												<X size={15} />
											) : (
												<Circle size={19} />
											)}
										</span>
										<strong>{name}</strong>
										<span>
											{run ? state(run.status, run.conclusion) : "Sin ejecución reciente"}
										</span>
										{run && <small>{run.head_sha?.slice(0, 7)}</small>}
									</button>
									{run && selected === run.id && (
										<div className="github-jobs">
											{jobError ||
												(jobs.length
													? jobs.map((job) => (
															<div key={job.id}>
																<strong>{job.name}</strong> · {state(job.status, job.conclusion)}
																{job.steps?.map((step, index) => (
																	<div key={index}>
																		{step.name} · {state(step.status, step.conclusion)}
																	</div>
																))}
															</div>
														))
													: jobsLoaded
														? "Sin trabajos disponibles"
														: "Cargando pasos…")}
										</div>
									)}
								</div>
							))}
					<div className="github-verification">
						{verification?.verified ? <ShieldCheck size={21} /> : <Circle size={21} />}
						<strong>Canal y firma</strong>
						<span>
							{verification?.verified ? `Verificada · ${verification.version}` : "Sin comprobar"}
						</span>
						<button
							disabled={verifying}
							onClick={() => void verifyRelease()}
							title="Verificar firma del instalador latest"
						>
							{verifying ? <LoaderCircle className="github-spin" size={16} /> : "Comprobar"}
						</button>
					</div>
					{verification?.verified && (
						<p className="github-foot">
							{verification.detail} ({Math.ceil(verification.bytes / 1_048_576)} MB).
						</p>
					)}
					{verificationError && (
						<p className="github-error" role="alert">
							{verificationError}
						</p>
					)}
					{names.length > 3 && (
						<button className="github-other" onClick={() => setShowOther((v) => !v)}>
							{showOther ? "Ocultar otras ejecuciones" : "Otras ejecuciones"}
						</button>
					)}
					<p className="github-foot">
						{accountView && account.connected ? `Cuenta · ${account.login}` : "Vista pública"}
						{updated && ` · Actualizado ${new Date(updated).toLocaleTimeString()}`}
					</p>
				</div>
			</div>
		</section>
	);
}
