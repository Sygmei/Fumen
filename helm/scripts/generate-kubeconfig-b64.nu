def main [
  --namespace (-n): string = "fumen"
  --secret (-s): string = "ci-helm-token"
  --server: string = "<SERVER_URL>"
  --cluster: string = "fumen-cluster"
  --context: string = "fumen-context"
  --user: string = "ci-helm"
] {
  if $server == "<SERVER_URL>" {
    error make { msg: "Pass --server with your Kubernetes API server URL." }
  }

  let token = (
    kubectl get secret $secret --namespace $namespace -o 'jsonpath={.data.token}'
    | decode base64
    | decode utf-8
    | str trim
  )

  let ca = (
    kubectl get secret $secret --namespace $namespace -o 'jsonpath={.data.ca\.crt}'
    | str trim
  )

  if ($token | is-empty) {
    error make { msg: $"Secret ($secret) does not contain a token yet. Wait a few seconds and try again." }
  }

  if ($ca | is-empty) {
    error make { msg: $"Secret ($secret) does not contain ca.crt yet. Wait a few seconds and try again." }
  }

  let kubeconfig = $"apiVersion: v1
kind: Config
clusters:
- cluster:
    certificate-authority-data: ($ca)
    server: ($server)
  name: ($cluster)
contexts:
- context:
    cluster: ($cluster)
    namespace: ($namespace)
    user: ($user)
  name: ($context)
current-context: ($context)
users:
- name: ($user)
  user:
    token: ($token)
"

  $kubeconfig | encode base64
}
