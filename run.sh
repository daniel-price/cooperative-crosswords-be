#

if docker container ls -a | grep crosswords ; then
   docker start crosswords 
else
    docker run --name crosswords -e POSTGRES_PASSWORD=password -p 5432:5432 postgres 
    ./build.sh
fi

cargo run
