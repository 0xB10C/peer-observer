import ipaddress

for ip in ipaddress.IPv4Network('117.128.0.0/10'):
    print(ip)
