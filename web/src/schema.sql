CREATE TABLE departments (
    id   serial PRIMARY KEY,
    name text NOT NULL UNIQUE
);

CREATE TABLE employees (
    id            serial PRIMARY KEY,
    name          text NOT NULL,
    email         text UNIQUE,
    department_id int REFERENCES departments,
    manager_id    int REFERENCES employees,
    salary        numeric(10,2) NOT NULL,
    hired_on      date NOT NULL,
    active        boolean NOT NULL DEFAULT true
);

CREATE TABLE projects (
    id            serial PRIMARY KEY,
    name          text NOT NULL,
    department_id int REFERENCES departments
);
