# do-updater

A dynamic DNS client for Kubernetes on DigitalOcean.

This utility allows you to maintain DNS records for Kubernetes nodes on
DigitalOcean without using a (relatively speaking) expensive load balancer.

## Why?

A regular Kubernetes `LoadBalancer` on DOKS is backed by a $10 USD/month
DigitalOcean loadbalancer. This could well be a significant portion of your
entire cluster cost for a small project using a couple of their cheapest $10/m
nodes.

Alternatively, you can allow clients to connect directly to your DOKS nodes and
let e.g. ingress-nginx route traffic to your services. This obviously loses out
on HA guarantees, but realistically small personal projects on tiny clusters
neither need nor benefit from this.

Unfortunately, static IPs on DigitalOcean also cost extra, even though your
nodes come with one for free. `do-updater` solves this by automatically
maintaining DNS records for your nodes via DigitalOcean's DNS API.

## Usage

Install on your cluster using Helm. First, clone the repository:

```bash
git clone https://github.com/timothyb89/do-updater
cd do-updater
```

Then, create a namespace and a secret containing a DigitalOcean API key:

```
kubectl create namespace do-updater
kubectl -n do-updater create secret generic do-updater --from-literal=key=supersecret
```

Next, create some minimal values in `values.yaml`:

```yaml
image:
  tag: v1.0.1

domain: example.com
dropletTag: k8s
record:
  kind: A
  ttl: 1800
  name: '*'
apiKey:
  secretName: do-updater
  secretKey: key
```

Lastly, install the service:

```bash
helm install -n do-updater do-updater ./chart -f values.yaml
```
