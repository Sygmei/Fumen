import type { AccessTokenRefreshResponse, AdminEnsembleResponse, AdminMusicPlaytimeResponse, AdminMusicProcessingLogResponse, AdminMusicProcessingProgressResponse, AdminMusicResponse, AdminRetryMusicProcessingRequest, AdminUpdateMusicMultipartRequest, AdminUpdateUserMultipartRequest, AdminUploadMusicMultipartRequest, AdminUserMetadataResponse, AdminUserScorePlaytimeResponse, AuthTokenResponse, CreateEnsembleRequest, CreateScoreAnnotationRequest, CreateUserRequest, CurrentUserResponse, DrumMapEntry, EnsembleMemberResponse, ErrorResponse, ExchangeLoginTokenRequest, HealthResponse, JsonValue, LoginLinkResponse, MoveMusicRequest, MusicPlaytimeLeaderboardEntryResponse, MusicPlaytimeTrackSummaryResponse, PublicMusicResponse, RefreshTokenRequest, ReportPlaytimeRequest, ScoreAnnotationListResponse, ScoreAnnotationResponse, StemInfo, TrackPlaytimeIncrementRequest, UpdateEnsembleMemberItemRequest, UpdateEnsembleMemberRequest, UpdateEnsembleMembersRequest, UpdateEnsembleScoresRequest, UpdateMusicEnsemblesRequest, UpdateMyProfileMultipartRequest, UserLibraryEnsembleResponse, UserLibraryResponse, UserLibraryScoreResponse, UserResponse } from "./models";

export type ErrorHandler = (response: globalThis.Response) => void | Promise<void>;

export interface ApiClientOptions {
  baseUrl: string;
  fetch?: typeof globalThis.fetch;
  headers?: globalThis.HeadersInit;
  onError?: ErrorHandler;
  validateContentEncodings?: boolean;
}

export interface RequestOptions extends Omit<globalThis.RequestInit, "method" | "body" | "headers"> {
  headers?: globalThis.HeadersInit;
  query?: globalThis.URLSearchParams | Record<string, string | number | boolean | undefined | null>;
  cookies?: Record<string, string>;
  onError?: ErrorHandler;
  validateContentEncodings?: boolean;
}

export class ApiClient {
  private readonly baseUrl: string;
  private readonly fetchFn: typeof globalThis.fetch;
  private readonly defaultHeaders: globalThis.Headers;
  private readonly onError: ErrorHandler;
  private readonly validateContentEncodings: boolean;


  constructor(options: ApiClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    this.fetchFn = options.fetch ?? globalThis.fetch;
    this.defaultHeaders = new globalThis.Headers(options.headers);
    this.validateContentEncodings = options.validateContentEncodings ?? true;
    this.onError = options.onError ?? ((response: globalThis.Response) => {
      throw new Error(`Request failed with status ${response.status}: ${response.statusText}`);
    });
  }

  private shouldValidateContentEncodings(requestOptions?: RequestOptions): boolean {
    return requestOptions?.validateContentEncodings ?? this.validateContentEncodings;
  }

  private validateStringEncoding(
    value: unknown,
    encoding: string,
    context: string,
    requestOptions?: RequestOptions,
  ): void {
    if (!this.shouldValidateContentEncodings(requestOptions) || value === undefined || value === null) {
      return;
    }
    if (encoding !== "base64") {
      return;
    }
    if (typeof value !== "string") {
      throw new Error(`${context} must be a base64 string`);
    }

    const candidate = value.trim();
    const base64Pattern = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
    if (candidate.length === 0 || candidate.length % 4 !== 0 || !base64Pattern.test(candidate)) {
      throw new Error(`${context} must be valid base64`);
    }
  }

  private createHeaders(): globalThis.Headers {
    return new globalThis.Headers(this.defaultHeaders);
  }

  private mergeHeaders(headers: globalThis.Headers | undefined, requestOptions?: RequestOptions): globalThis.Headers | undefined {
    const merged = headers ? new globalThis.Headers(headers) : new globalThis.Headers(this.defaultHeaders);

    if (requestOptions?.headers) {
      const overrideHeaders = new globalThis.Headers(requestOptions.headers);
      for (const [key, value] of overrideHeaders.entries()) {
        merged.set(key, value);
      }
    }

    if (requestOptions?.cookies && Object.keys(requestOptions.cookies).length > 0) {
      const cookieHeader = Object.entries(requestOptions.cookies)
        .map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(value)}`)
        .join("; ");
      if (cookieHeader.length > 0) {
        merged.set("Cookie", cookieHeader);
      }
    }

    return Array.from(merged.keys()).length > 0 ? merged : undefined;
  }

  private mergeQuery(
    baseQuery: globalThis.URLSearchParams | undefined,
    requestOptions?: RequestOptions,
  ): globalThis.URLSearchParams | undefined {
    const merged = baseQuery ? new globalThis.URLSearchParams(baseQuery) : new globalThis.URLSearchParams();

    if (requestOptions?.query instanceof globalThis.URLSearchParams) {
      for (const [key, value] of requestOptions.query.entries()) {
        merged.set(key, value);
      }
    } else if (requestOptions?.query) {
      for (const [key, value] of Object.entries(requestOptions.query)) {
        if (value !== undefined && value !== null) {
          merged.set(key, String(value));
        }
      }
    }

    return Array.from(merged.keys()).length > 0 ? merged : undefined;
  }

  private createRequestInit(requestOptions?: RequestOptions): globalThis.RequestInit {
    if (!requestOptions) {
      return {};
    }

    const { headers: _headers, query: _query, cookies: _cookies, onError: _onError, ...requestInit } = requestOptions;
    return { ...requestInit };
  }

  private async handleError(response: globalThis.Response, requestOptions?: RequestOptions): Promise<void> {
    if (!response.ok) {
      await (requestOptions?.onError ?? this.onError)(response);
    }
  }

  private buildUrl(path: string, query?: globalThis.URLSearchParams): string {
    const url = new globalThis.URL(`${this.baseUrl}${path}`);
    if (query) {
      url.search = query.toString();
    }
    return url.toString();
  }

  private async parseResponse<T>(
    response: globalThis.Response,
    responseEncoding?: string,
    requestOptions?: RequestOptions,
  ): Promise<T> {
    if (response.status === 204) {
      return undefined as T;
    }

    const contentType = response.headers.get("content-type") ?? "";
    if (contentType.includes("application/json")) {
      const payload = await response.json();
      if (responseEncoding) {
        this.validateStringEncoding(payload, responseEncoding, "response body", requestOptions);
      }
      return payload as T;
    }

    const payload = await response.text();
    if (responseEncoding) {
      this.validateStringEncoding(payload, responseEncoding, "response body", requestOptions);
    }
    return payload as T;
  }

  private toFormData(payload: Record<string, unknown>): FormData {
    const formData = new FormData();
    for (const [key, value] of Object.entries(payload)) {
      if (value === undefined || value === null) {
        continue;
      }
      if (Array.isArray(value)) {
        for (const item of value) {
          if (item !== undefined && item !== null) {
            formData.append(key, item as string | Blob);
          }
        }
        continue;
      }
      formData.append(key, value as string | Blob);
    }
    return formData;
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   * @param ensembleId Ensemble identifier
   */
  async _adminAddMusicToEnsembleRaw(id: string, ensembleId: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/ensembles/${ensembleId}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   * @param ensembleId Ensemble identifier
   */
  async adminAddMusicToEnsemble(id: string, ensembleId: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminAddMusicToEnsembleRaw(id, ensembleId, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Ensemble identifier
   * @param userId User identifier
   */
  async _adminAddUserToEnsembleRaw(id: string, userId: string, body: UpdateEnsembleMemberRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/ensembles/${id}/users/${userId}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Ensemble identifier
   * @param userId User identifier
   */
  async adminAddUserToEnsemble(id: string, userId: string, body: UpdateEnsembleMemberRequest, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminAddUserToEnsembleRaw(id, userId, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  async _adminCreateEnsembleRaw(body: CreateEnsembleRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/ensembles`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async adminCreateEnsemble(body: CreateEnsembleRequest, requestOptions?: RequestOptions): Promise<AdminEnsembleResponse> {
    const response = await this._adminCreateEnsembleRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminEnsembleResponse>(response, undefined, requestOptions);
  }



  async _adminCreateUserRaw(body: CreateUserRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/users`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async adminCreateUser(body: CreateUserRequest, requestOptions?: RequestOptions): Promise<UserResponse> {
    const response = await this._adminCreateUserRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<UserResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id User identifier
   */
  async _adminCreateUserLoginLinkRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/users/${id}/login-link`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id User identifier
   */
  async adminCreateUserLoginLink(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<LoginLinkResponse> {
    const response = await this._adminCreateUserLoginLinkRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<LoginLinkResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Ensemble identifier
   */
  async _adminDeleteEnsembleRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/ensembles/${id}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "DELETE";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Ensemble identifier
   */
  async adminDeleteEnsemble(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminDeleteEnsembleRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminDeleteMusicRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/delete`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminDeleteMusic(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminDeleteMusicRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id User identifier
   */
  async _adminDeleteUserRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/users/${id}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "DELETE";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id User identifier
   */
  async adminDeleteUser(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminDeleteUserRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  async _adminListEnsemblesRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/ensembles`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async adminListEnsembles(body?: unknown, requestOptions?: RequestOptions): Promise<AdminEnsembleResponse[]> {
    const response = await this._adminListEnsemblesRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminEnsembleResponse[]>(response, undefined, requestOptions);
  }



  async _adminListMusicsRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async adminListMusics(body?: unknown, requestOptions?: RequestOptions): Promise<AdminMusicResponse[]> {
    const response = await this._adminListMusicsRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicResponse[]>(response, undefined, requestOptions);
  }



  async _adminListUsersRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/users`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async adminListUsers(body?: unknown, requestOptions?: RequestOptions): Promise<UserResponse[]> {
    const response = await this._adminListUsersRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<UserResponse[]>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminMoveMusicRaw(id: string, body: MoveMusicRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/move`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminMoveMusic(id: string, body: MoveMusicRequest, requestOptions?: RequestOptions): Promise<AdminMusicResponse> {
    const response = await this._adminMoveMusicRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminMusicPlaytimeRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/playtime`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminMusicPlaytime(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<AdminMusicPlaytimeResponse> {
    const response = await this._adminMusicPlaytimeRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicPlaytimeResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminMusicProcessingLogRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/processing-log`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminMusicProcessingLog(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<AdminMusicProcessingLogResponse> {
    const response = await this._adminMusicProcessingLogRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicProcessingLogResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminMusicProcessingProgressRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/processing-progress`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminMusicProcessingProgress(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<AdminMusicProcessingProgressResponse> {
    const response = await this._adminMusicProcessingProgressRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicProcessingProgressResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   * @param ensembleId Ensemble identifier
   */
  async _adminRemoveMusicFromEnsembleRaw(id: string, ensembleId: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/ensembles/${ensembleId}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "DELETE";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   * @param ensembleId Ensemble identifier
   */
  async adminRemoveMusicFromEnsemble(id: string, ensembleId: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminRemoveMusicFromEnsembleRaw(id, ensembleId, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Ensemble identifier
   * @param userId User identifier
   */
  async _adminRemoveUserFromEnsembleRaw(id: string, userId: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/ensembles/${id}/users/${userId}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "DELETE";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Ensemble identifier
   * @param userId User identifier
   */
  async adminRemoveUserFromEnsemble(id: string, userId: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminRemoveUserFromEnsembleRaw(id, userId, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminRetryRenderRaw(id: string, body?: AdminRetryMusicProcessingRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/retry`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    if (body !== undefined) {
      headers.set("Content-Type", "application/json");
    }
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    if (body !== undefined) {
      requestInit.body = JSON.stringify(body);
    }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminRetryRender(id: string, body?: AdminRetryMusicProcessingRequest, requestOptions?: RequestOptions): Promise<AdminMusicResponse> {
    const response = await this._adminRetryRenderRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Ensemble identifier
   */
  async _adminUpdateEnsembleMembersRaw(id: string, body: UpdateEnsembleMembersRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/ensembles/${id}/users`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "PATCH";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Ensemble identifier
   */
  async adminUpdateEnsembleMembers(id: string, body: UpdateEnsembleMembersRequest, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminUpdateEnsembleMembersRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Ensemble identifier
   */
  async _adminUpdateEnsembleScoresRaw(id: string, body: UpdateEnsembleScoresRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/ensembles/${id}/scores`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "PATCH";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Ensemble identifier
   */
  async adminUpdateEnsembleScores(id: string, body: UpdateEnsembleScoresRequest, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminUpdateEnsembleScoresRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminUpdateMusicRaw(id: string, body: AdminUpdateMusicMultipartRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "PATCH";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = this.toFormData(body as unknown as Record<string, unknown>);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminUpdateMusic(id: string, body: AdminUpdateMusicMultipartRequest, requestOptions?: RequestOptions): Promise<AdminMusicResponse> {
    const response = await this._adminUpdateMusicRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id Music identifier
   */
  async _adminUpdateMusicEnsemblesRaw(id: string, body: UpdateMusicEnsemblesRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics/${id}/ensembles`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "PATCH";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id Music identifier
   */
  async adminUpdateMusicEnsembles(id: string, body: UpdateMusicEnsemblesRequest, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._adminUpdateMusicEnsemblesRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id User identifier
   */
  async _adminUpdateUserRaw(id: string, body: AdminUpdateUserMultipartRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/users/${id}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "PATCH";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = this.toFormData(body as unknown as Record<string, unknown>);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id User identifier
   */
  async adminUpdateUser(id: string, body: AdminUpdateUserMultipartRequest, requestOptions?: RequestOptions): Promise<UserResponse> {
    const response = await this._adminUpdateUserRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<UserResponse>(response, undefined, requestOptions);
  }



  async _adminUploadMusicRaw(body: AdminUploadMusicMultipartRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/musics`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = this.toFormData(body as unknown as Record<string, unknown>);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async adminUploadMusic(body: AdminUploadMusicMultipartRequest, requestOptions?: RequestOptions): Promise<AdminMusicResponse> {
    const response = await this._adminUploadMusicRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminMusicResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param id User identifier
   */
  async _adminUserMetadataRaw(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/admin/users/${id}/metadata`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param id User identifier
   */
  async adminUserMetadata(id: string, body?: unknown, requestOptions?: RequestOptions): Promise<AdminUserMetadataResponse> {
    const response = await this._adminUserMetadataRaw(id, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AdminUserMetadataResponse>(response, undefined, requestOptions);
  }



  async _createMyLoginLinkRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/me/login-link`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async createMyLoginLink(body?: unknown, requestOptions?: RequestOptions): Promise<LoginLinkResponse> {
    const response = await this._createMyLoginLinkRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<LoginLinkResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _createPublicMusicAnnotationRaw(accessKey: string, body: CreateScoreAnnotationRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/annotations`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async createPublicMusicAnnotation(accessKey: string, body: CreateScoreAnnotationRequest, requestOptions?: RequestOptions): Promise<ScoreAnnotationResponse> {
    const response = await this._createPublicMusicAnnotationRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<ScoreAnnotationResponse>(response, undefined, requestOptions);
  }



  async _currentUserRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/me`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async currentUser(body?: unknown, requestOptions?: RequestOptions): Promise<CurrentUserResponse> {
    const response = await this._currentUserRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<CurrentUserResponse>(response, undefined, requestOptions);
  }



  async _currentUserLibraryRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/me/library`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async currentUserLibrary(body?: unknown, requestOptions?: RequestOptions): Promise<UserLibraryResponse> {
    const response = await this._currentUserLibraryRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<UserLibraryResponse>(response, undefined, requestOptions);
  }



  async _exchangeLoginTokenRaw(body: ExchangeLoginTokenRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/auth/exchange`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async exchangeLoginToken(body: ExchangeLoginTokenRequest, requestOptions?: RequestOptions): Promise<AuthTokenResponse> {
    const response = await this._exchangeLoginTokenRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AuthTokenResponse>(response, undefined, requestOptions);
  }



  async _healthRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/health`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async health(body?: unknown, requestOptions?: RequestOptions): Promise<HealthResponse> {
    const response = await this._healthRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<HealthResponse>(response, undefined, requestOptions);
  }



  async _meLogoutRaw(body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/me/logout`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async meLogout(body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._meLogoutRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusic(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<PublicMusicResponse> {
    const response = await this._publicMusicRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<PublicMusicResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicAnnotationsRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/annotations`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusicAnnotations(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<ScoreAnnotationListResponse> {
    const response = await this._publicMusicAnnotationsRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<ScoreAnnotationListResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicAudioRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/audio`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusicAudio(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._publicMusicAudioRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicDownloadRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/download`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusicDownload(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._publicMusicDownloadRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicIconRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/icon`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusicIcon(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._publicMusicIconRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicMidiRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/midi`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusicMidi(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._publicMusicMidiRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicMusicxmlRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/musicxml`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusicMusicxml(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._publicMusicMusicxmlRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   * @param trackIndex Stem track index
   */
  async _publicMusicStemAudioRaw(accessKey: string, trackIndex: number, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/stems/${trackIndex}`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   * @param trackIndex Stem track index
   */
  async publicMusicStemAudio(accessKey: string, trackIndex: number, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._publicMusicStemAudioRaw(accessKey, trackIndex, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _publicMusicStemsRaw(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/stems`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async publicMusicStems(accessKey: string, body?: unknown, requestOptions?: RequestOptions): Promise<StemInfo[]> {
    const response = await this._publicMusicStemsRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<StemInfo[]>(response, undefined, requestOptions);
  }



  async _refreshAccessTokenRaw(body: RefreshTokenRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/auth/refresh`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async refreshAccessToken(body: RefreshTokenRequest, requestOptions?: RequestOptions): Promise<AccessTokenRefreshResponse> {
    const response = await this._refreshAccessTokenRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<AccessTokenRefreshResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param accessKey Public score token or public id
   */
  async _reportPublicMusicPlaytimeRaw(accessKey: string, body: ReportPlaytimeRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/public/${accessKey}/playtime`;
    const query = this.mergeQuery(undefined, requestOptions);
    const headers = this.createHeaders();
    headers.set("Content-Type", "application/json");
    const mergedHeaders = this.mergeHeaders(headers, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "POST";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = JSON.stringify(body);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param accessKey Public score token or public id
   */
  async reportPublicMusicPlaytime(accessKey: string, body: ReportPlaytimeRequest, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._reportPublicMusicPlaytimeRaw(accessKey, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }



  async _updateMyProfileRaw(body: UpdateMyProfileMultipartRequest, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/me/profile`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "PATCH";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    requestInit.body = this.toFormData(body as unknown as Record<string, unknown>);
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }
  async updateMyProfile(body: UpdateMyProfileMultipartRequest, requestOptions?: RequestOptions): Promise<CurrentUserResponse> {
    const response = await this._updateMyProfileRaw(body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<CurrentUserResponse>(response, undefined, requestOptions);
  }



  /**
   * Returns the raw HTTP response without parsing it or throwing for HTTP errors.
   * @param userId User identifier
   */
  async _userAvatarRaw(userId: string, body?: unknown, requestOptions?: RequestOptions): Promise<Response> {
    const path = `/api/users/${userId}/avatar`;
    const query = this.mergeQuery(undefined, requestOptions);
    const mergedHeaders = this.mergeHeaders(undefined, requestOptions);
    const requestInit = this.createRequestInit(requestOptions);
    requestInit.method = "GET";
    if (mergedHeaders) { requestInit.headers = mergedHeaders; }
    const response = await this.fetchFn(this.buildUrl(path, query), requestInit);
    return response;
  }

  /**
   * @param userId User identifier
   */
  async userAvatar(userId: string, body?: unknown, requestOptions?: RequestOptions): Promise<unknown> {
    const response = await this._userAvatarRaw(userId, body, requestOptions);
    await this.handleError(response, requestOptions);
    return await this.parseResponse<unknown>(response, undefined, requestOptions);
  }


}
