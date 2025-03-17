FROM ubuntu:24.04

# Install curl
RUN apt-get update && apt-get install -y \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -s /bin/bash helios

# Switch to non-root user
USER helios
WORKDIR /home/helios

# Set environment variable for helios
ENV PATH="/home/helios/.helios/bin:${PATH}"

# Install heliosup first, then use it to install helios
RUN curl -fsSL https://raw.githubusercontent.com/a16z/helios/master/heliosup/install | bash && \
    /bin/bash -c 'heliosup'

# Set a default command
CMD ["/bin/bash"]
